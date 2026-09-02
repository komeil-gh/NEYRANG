use std::{
    fs,
    path::PathBuf,
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

use neyrang::chess::Position;
use neyrang_nnue_data::{Game, GameResult, ScoredMove, encode_games};
use neyrang_nnue_reference::{HIDDEN_SIZE, INPUT_FEATURES, Network, NetworkParameters};

#[test]
fn diagnose_sign_emits_reproducible_sliced_evidence() {
    let directory = unique_temp_directory();
    fs::create_dir(&directory).unwrap();
    let baseline_path = directory.join("baseline.nnue");
    let candidate_path = directory.join("candidate.nnue");
    let corpus_path = directory.join("validation.vf");
    fs::write(&baseline_path, constant_network(1).to_bytes()).unwrap();
    fs::write(&candidate_path, constant_network(-1).to_bytes()).unwrap();
    fs::write(&corpus_path, encode_games(&[scored_game()]).unwrap()).unwrap();

    let first = Command::new(env!("CARGO_BIN_EXE_diagnose-sign"))
        .arg(&baseline_path)
        .arg(&candidate_path)
        .arg(&corpus_path)
        .arg("2026090203000001")
        .arg("1000")
        .output()
        .unwrap();
    let repeat = Command::new(env!("CARGO_BIN_EXE_diagnose-sign"))
        .arg(&baseline_path)
        .arg(&candidate_path)
        .arg(&corpus_path)
        .arg("2026090203000001")
        .arg("1000")
        .output()
        .unwrap();

    assert!(first.status.success());
    assert_eq!(first.stdout, repeat.stdout);
    let stdout = String::from_utf8(first.stdout).unwrap();
    assert!(stdout.contains("\"schema\":\"neyrang-nnue-sign-diagnostics-v1\""));
    assert!(stdout.contains("\"signed_games\":1"));
    assert!(stdout.contains("\"baseline_only_correct\":1"));
    assert!(stdout.contains("\"score_magnitude_bands\":"));
    assert!(stdout.contains("\"king_bucket_pairs\":"));
    assert!(stdout.contains("\"contexts\":"));
    assert!(stdout.contains("\"bootstrap_seed\":2026090203000001"));
    assert!(stdout.contains("\"bootstrap_replicates\":1000"));

    fs::remove_dir_all(directory).unwrap();
}

fn constant_network(output_bias: i32) -> Network {
    Network::new(
        NetworkParameters {
            activation_quant: 1,
            output_quant: 1,
            centipawn_scale: 400,
        },
        vec![0; INPUT_FEATURES * HIDDEN_SIZE],
        vec![0; HIDDEN_SIZE],
        vec![0; 2 * HIDDEN_SIZE],
        output_bias,
    )
    .unwrap()
}

fn scored_game() -> Game {
    let mut initial_position = Position::startpos();
    let mv = initial_position.find_legal_move("e2e4").unwrap();
    Game {
        initial_position,
        header_score: 0,
        result: GameResult::Draw,
        extra: 0,
        moves: vec![ScoredMove { mv, score_cp: 400 }],
    }
}

fn unique_temp_directory() -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!(
        "neyrang-nnue-diagnose-sign-{}-{nonce}",
        std::process::id()
    ))
}
