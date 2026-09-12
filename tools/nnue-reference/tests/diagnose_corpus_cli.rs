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
fn diagnose_corpus_emits_reproducible_machine_readable_evidence() {
    for score in [0, 29999] {
        let directory = unique_temp_directory();
        fs::create_dir(&directory).unwrap();
        let network_path = directory.join("network.nnue");
        let corpus_path = directory.join("validation.vf");
        fs::write(&network_path, constant_network().to_bytes()).unwrap();
        let mut game = draw_game();
        game.moves[0].score_cp = score;
        fs::write(&corpus_path, encode_games(&[game]).unwrap()).unwrap();

        let first = Command::new(env!("CARGO_BIN_EXE_diagnose-corpus"))
            .arg(&network_path)
            .arg(&corpus_path)
            .output()
            .unwrap();
        let repeat = Command::new(env!("CARGO_BIN_EXE_diagnose-corpus"))
            .arg(&network_path)
            .arg(&corpus_path)
            .output()
            .unwrap();

        assert!(first.status.success());
        assert_eq!(first.stdout, repeat.stdout);
        let stdout = String::from_utf8(first.stdout).unwrap();
        assert!(stdout.contains("\"schema\":\"neyrang-nnue-search-diagnostics-v2\""));
        assert!(stdout.contains("\"teacher_mate_abs_min\":29872"));
        assert!(stdout.contains(&format!("\"teacher_score_range_raw\":[{score},{score}]")));
        assert!(stdout.contains("\"sign_agreement_rate\":null"));
        assert!(!stdout.contains("NaN"));
        if score != 0 {
            assert!(
                stdout.contains("\"network_vs_search\":{\"samples\":0,\"reference_mean_cp\":null")
            );
            assert!(stdout.contains("\"mate_samples\":1,\"mate_sign_agreements\":0"));
        }
        assert!(stdout.contains("\"games\":1"));
        assert!(stdout.contains("\"positions\":1"));
        assert!(stdout.contains("\"network_vs_search\":"));
        assert!(stdout.contains("\"classical_vs_search\":"));
        assert!(stdout.contains("\"blended_probability_mse\":"));
        assert!(stdout.contains("\"composition\":"));
        assert!(stdout.contains("\"bullet_default_rejected\":1"));

        fs::remove_dir_all(directory).unwrap();
    }
}

fn constant_network() -> Network {
    Network::new(
        NetworkParameters {
            activation_quant: 1,
            output_quant: 1,
            centipawn_scale: 400,
        },
        vec![0; INPUT_FEATURES * HIDDEN_SIZE],
        vec![0; HIDDEN_SIZE],
        vec![0; 2 * HIDDEN_SIZE],
        0,
    )
    .unwrap()
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
        "neyrang-nnue-diagnose-corpus-{}-{nonce}",
        std::process::id()
    ))
}
