use neyrang::chess::{Position, zobrist};

#[test]
fn incremental_hash_and_position_restore_after_double_pawn_push() {
    let mut position = Position::startpos();
    let original = position.clone();
    let mv = position
        .find_legal_move("e2e4")
        .expect("e2e4 is legal in the starting position");

    let undo = position.make_move(mv);
    assert_eq!(position.hash(), zobrist::recompute(&position));
    position
        .verify_integrity()
        .expect("made position is coherent");

    position.unmake_move(mv, undo);
    assert_eq!(position, original);
    position
        .verify_integrity()
        .expect("restored position is coherent");
}

#[test]
fn repetition_hash_ignores_an_uncapturable_en_passant_target() {
    let mut with_target =
        Position::from_fen("7k/8/8/8/4P3/8/8/K7 b - e3 0 1").expect("fixture is valid");
    let mut without_target =
        Position::from_fen("7k/8/8/8/4P3/8/8/K7 b - - 0 1").expect("fixture is valid");

    assert_ne!(with_target.hash(), without_target.hash());
    assert_eq!(
        with_target.repetition_hash(),
        without_target.repetition_hash()
    );
}

#[test]
fn repetition_hash_keeps_a_legal_en_passant_target() {
    let mut with_target =
        Position::from_fen("7k/8/8/8/3pP3/8/8/K7 b - e3 0 1").expect("fixture is valid");
    let mut without_target =
        Position::from_fen("7k/8/8/8/3pP3/8/8/K7 b - - 0 1").expect("fixture is valid");

    assert!(
        with_target
            .legal_moves()
            .iter()
            .any(|mv| mv.is_en_passant())
    );
    assert_ne!(
        with_target.repetition_hash(),
        without_target.repetition_hash()
    );
}

#[test]
fn repetition_hash_ignores_a_king_pinned_en_passant_target() {
    let mut with_target =
        Position::from_fen("4r2k/8/8/3pP3/8/8/8/4K3 w - d6 0 1").expect("fixture is valid");
    let mut without_target =
        Position::from_fen("4r2k/8/8/3pP3/8/8/8/4K3 w - - 0 1").expect("fixture is valid");

    assert!(
        with_target
            .legal_moves()
            .iter()
            .all(|mv| !mv.is_en_passant())
    );
    assert_eq!(
        with_target.repetition_hash(),
        without_target.repetition_hash()
    );
}
