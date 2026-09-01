use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

use neyrang_nnue_data::decode_games;

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new(label: &str) -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock follows the Unix epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "neyrang-nnue-selfplay-{label}-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir(&path).expect("test directory can be created");
        Self(path)
    }

    fn join(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).expect("test directory can be removed");
    }
}

fn run_generator(openings: &Path, output: &Path, summary: &Path) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_generate-selfplay"))
        .args([
            "--openings",
            openings.to_str().unwrap(),
            "--output",
            output.to_str().unwrap(),
            "--summary",
            summary.to_str().unwrap(),
            "--nodes",
            "256",
            "--hash-mb",
            "1",
            "--max-plies",
            "8",
        ])
        .output()
        .expect("self-play generator starts")
}

#[test]
fn cli_records_a_complete_game_and_machine_readable_reason_counts() {
    let directory = TestDirectory::new("accepted");
    let openings = directory.join("openings.epd");
    let output = directory.join("games.vf");
    let summary = directory.join("summary.json");
    fs::write(&openings, "7k/5K2/6Q1/8/8/8/8/8 w - - 0 1\n").unwrap();

    let result = run_generator(&openings, &output, &summary);

    assert!(
        result.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    let games = decode_games(&fs::read(&output).unwrap()).expect("output must replay");
    assert_eq!(games.len(), 1);
    assert_eq!(games[0].moves.len(), 1);
    let report = fs::read_to_string(summary).unwrap();
    assert!(report.contains("\"schema\": \"neyrang-selfplay-summary-v1\""));
    assert!(report.contains("\"attempted_games\": 1"));
    assert!(report.contains("\"accepted_games\": 1"));
    assert!(report.contains("\"checkmates\": 1"));
}

#[test]
fn cli_rejects_bad_input_and_never_overwrites_an_existing_artifact() {
    let directory = TestDirectory::new("rejected");
    let openings = directory.join("openings.epd");
    let output = directory.join("games.vf");
    let summary = directory.join("summary.json");
    fs::write(&openings, "not a fen\n").unwrap();
    fs::write(&output, b"preserve me").unwrap();

    let result = run_generator(&openings, &output, &summary);

    assert!(!result.status.success());
    assert_eq!(fs::read(&output).unwrap(), b"preserve me");
    assert!(!summary.exists());
}
