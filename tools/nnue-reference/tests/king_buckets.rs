use neyrang::chess::Position;
use neyrang_nnue_data::{Game, GameResult, ScoredMove};
use neyrang_nnue_reference::analyze_king_buckets;

#[test]
fn king_bucket_analysis_counts_both_oriented_perspectives() {
    let mut position = Position::from_fen("4k3/8/8/8/8/8/8/K7 w - - 0 1").unwrap();
    let mv = position.find_legal_move("a1a2").unwrap();
    let game = Game {
        initial_position: position,
        header_score: 0,
        result: GameResult::Draw,
        extra: 0,
        moves: vec![ScoredMove { mv, score_cp: 0 }],
    };

    let report = analyze_king_buckets(&[game]).unwrap();

    assert_eq!(report.games, 1);
    assert_eq!(report.positions, 1);
    assert_eq!(report.perspective_samples, 2);
    assert_eq!(report.oriented_king_squares[0], 1); // White king on a1.
    assert_eq!(report.oriented_king_squares[4], 1); // Black e8 becomes e1.
    assert_eq!(report.horizontally_mirrored_king_squares[0], 1);
    assert_eq!(report.horizontally_mirrored_king_squares[3], 1);
    assert_eq!(
        report
            .horizontally_mirrored_king_squares
            .iter()
            .sum::<u64>(),
        2
    );
}

#[test]
fn king_bucket_analysis_rejects_empty_input_and_empty_games() {
    assert!(analyze_king_buckets(&[]).is_err());

    let empty = Game {
        initial_position: Position::startpos(),
        header_score: 0,
        result: GameResult::Draw,
        extra: 0,
        moves: Vec::new(),
    };
    assert!(analyze_king_buckets(&[empty]).is_err());
}
