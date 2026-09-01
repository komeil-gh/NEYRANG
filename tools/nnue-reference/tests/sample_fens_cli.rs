use std::{
    fs,
    path::PathBuf,
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

use neyrang::chess::{Color, Position};
use neyrang_nnue_data::{Game, GameResult, ScoredMove, encode_games};
use neyrang_nnue_reference::parse_fen_suite;

#[test]
fn sampler_is_deterministic_color_balanced_and_no_clobber() {
    let directory = unique_temp_directory();
    fs::create_dir(&directory).unwrap();
    let corpus_path = directory.join("corpus.vf");
    let first_path = directory.join("first.fen");
    let second_path = directory.join("second.fen");
    fs::write(&corpus_path, fixture_corpus()).unwrap();

    let first = sample(&first_path, &corpus_path);
    assert!(first.status.success());
    assert!(
        String::from_utf8(first.stdout)
            .unwrap()
            .contains("\"positions\":4")
    );
    let second = sample(&second_path, &corpus_path);
    assert!(second.status.success());
    assert_eq!(
        fs::read(&first_path).unwrap(),
        fs::read(&second_path).unwrap()
    );

    let suite = parse_fen_suite(&fs::read_to_string(&first_path).unwrap()).unwrap();
    assert_eq!(suite.len(), 4);
    assert_eq!(
        suite
            .iter()
            .filter(|item| item.position.side_to_move() == Color::White)
            .count(),
        2
    );
    assert_eq!(
        suite
            .iter()
            .filter(|item| item.position.side_to_move() == Color::Black)
            .count(),
        2
    );

    let clobber = sample(&first_path, &corpus_path);
    assert!(!clobber.status.success());
    assert!(
        String::from_utf8(clobber.stderr)
            .unwrap()
            .contains("refusing to overwrite")
    );

    fs::remove_dir_all(directory).unwrap();
}

fn sample(output: &PathBuf, corpus: &PathBuf) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_sample-fens"))
        .arg(output)
        .arg("4")
        .arg("n1d-test-seed")
        .arg(corpus)
        .output()
        .unwrap()
}

fn fixture_corpus() -> Vec<u8> {
    let initial_position = Position::startpos();
    let mut position = initial_position.clone();
    let mut moves = Vec::new();
    for notation in ["e2e4", "e7e5", "g1f3", "b8c6", "f1b5", "a7a6"] {
        let mv = position.find_legal_move(notation).unwrap();
        moves.push(ScoredMove { mv, score_cp: 0 });
        position.make_move(mv);
    }
    encode_games(&[Game {
        initial_position,
        header_score: 0,
        result: GameResult::Draw,
        extra: 0,
        moves,
    }])
    .unwrap()
}

fn unique_temp_directory() -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!(
        "neyrang-nnue-fen-sample-{}-{nonce}",
        std::process::id()
    ))
}
