use neyrang::chess::{Move, Position, zobrist};

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

#[test]
fn null_move_clears_en_passant_and_restores_exact_search_state() {
    let mut position = Position::from_fen("r3k2r/8/8/3pP3/8/8/8/R3K2R w KQkq d6 47 23")
        .expect("null-move fixture must be valid");
    let original_repetition_hash = position.repetition_hash();
    let original = position.clone();
    let original_hash = position.hash();
    let original_castling = position.castling_rights();
    let original_moves = position.legal_moves();

    let undo = position.make_null_move();

    assert_ne!(position.side_to_move(), original.side_to_move());
    assert_eq!(position.en_passant(), None);
    assert_eq!(position.halfmove_clock(), 47);
    assert_eq!(position.fullmove_number(), 23);
    assert_eq!(position.castling_rights(), original_castling);
    assert_ne!(position.hash(), original_hash);
    assert_eq!(position.hash(), zobrist::recompute(&position));
    assert!(original_moves.iter().all(|&mv| mv != Move::NONE));
    position
        .verify_integrity()
        .expect("null position must remain coherent");

    position.unmake_null_move(undo);

    assert_eq!(position, original);
    assert_eq!(position.repetition_hash(), original_repetition_hash);
    position
        .verify_integrity()
        .expect("unmade null position must restore every field");
}
