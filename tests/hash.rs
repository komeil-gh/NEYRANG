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
