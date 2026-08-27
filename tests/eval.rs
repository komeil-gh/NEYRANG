use neyrang::{
    chess::Position,
    eval::{TEMPO, evaluate},
};

#[test]
fn symmetric_position_scores_only_side_to_move_tempo() {
    let white_to_move = Position::startpos();
    let black_to_move =
        Position::from_fen("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR b KQkq - 0 1")
            .expect("symmetric fixture is valid");

    assert_eq!(evaluate(&white_to_move), TEMPO);
    assert_eq!(evaluate(&black_to_move), TEMPO);
}

#[test]
fn material_advantage_is_scored_for_the_side_that_owns_it() {
    let white =
        Position::from_fen("7k/8/8/8/8/8/4Q3/7K w - - 0 1").expect("material fixture is valid");
    let black =
        Position::from_fen("7k/8/8/8/8/8/4q3/7K w - - 0 1").expect("material fixture is valid");

    assert!(evaluate(&white) > 800);
    assert!(evaluate(&black) < -800);
}
