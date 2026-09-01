use std::{
    fs,
    path::PathBuf,
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

use neyrang::chess::Position;
use neyrang_nnue_data::{Game, GameResult, ScoredMove, encode_games};

#[test]
fn king_bucket_cli_emits_deterministic_machine_readable_counts() {
    let directory = unique_temp_directory();
    fs::create_dir(&directory).unwrap();
    let corpus_path = directory.join("corpus.vf");
    fs::write(&corpus_path, encode_games(&[draw_game()]).unwrap()).unwrap();

    let first = Command::new(env!("CARGO_BIN_EXE_analyze-king-buckets"))
        .arg(&corpus_path)
        .output()
        .unwrap();
    let repeat = Command::new(env!("CARGO_BIN_EXE_analyze-king-buckets"))
        .arg(&corpus_path)
        .output()
        .unwrap();

    assert!(first.status.success());
    assert_eq!(first.stdout, repeat.stdout);
    let stdout = String::from_utf8(first.stdout).unwrap();
    assert!(stdout.contains("\"schema\":\"neyrang-nnue-king-bucket-analysis-v1\""));
    assert!(stdout.contains("\"games\":1"));
    assert!(stdout.contains("\"positions\":1"));
    assert!(stdout.contains("\"perspective_samples\":2"));
    assert!(stdout.contains("\"oriented_king_squares\":["));
    assert!(stdout.contains("\"horizontally_mirrored_king_squares\":["));

    fs::remove_dir_all(directory).unwrap();
}

fn draw_game() -> Game {
    let mut initial_position = Position::startpos();
    let mv = initial_position.find_legal_move("e2e4").unwrap();
    Game {
        initial_position,
        header_score: 0,
        result: GameResult::Draw,
        extra: 0,
        moves: vec![ScoredMove { mv, score_cp: 0 }],
    }
}

fn unique_temp_directory() -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!(
        "neyrang-nnue-king-buckets-{}-{nonce}",
        std::process::id()
    ))
}
