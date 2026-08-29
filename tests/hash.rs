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
fn null_move_clears_en_passant_and_restores_exact_search_state() {
    let mut position = Position::from_fen("r3k2r/8/8/3pP3/8/8/8/R3K2R w KQkq d6 47 23")
        .expect("null-move fixture must be valid");
    let original = position.clone();
    let original_hash = position.hash();
    let original_castling = position.castling_rights();

    let undo = position.make_null_move();

    assert_ne!(position.side_to_move(), original.side_to_move());
    assert_eq!(position.en_passant(), None);
    assert_eq!(position.halfmove_clock(), 47);
    assert_eq!(position.fullmove_number(), 23);
    assert_eq!(position.castling_rights(), original_castling);
    assert_ne!(position.hash(), original_hash);
    assert_eq!(position.hash(), zobrist::recompute(&position));
    position
        .verify_integrity()
        .expect("null position must remain coherent");

    position.unmake_null_move(undo);

    assert_eq!(position, original);
    position
        .verify_integrity()
        .expect("unmade null position must restore every field");
}
