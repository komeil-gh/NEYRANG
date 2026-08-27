use neyrang::{
    chess::{Move, MoveFlag, Square},
    search::{
        VALUE_MATE,
        tt::{Bound, TranspositionTable},
    },
};

#[test]
fn transposition_table_normalizes_mate_scores_between_plies() {
    let mut table = TranspositionTable::new(1);
    let best = Move::new(Square::G6, Square::G7, MoveFlag::Quiet);
    let key = 0xDEAD_BEEF_CAFE_BABE;

    table.store(key, 8, VALUE_MATE - 7, Bound::Exact, best, 5);

    let same_ply = table.probe(key, 5).expect("entry should be present");
    let shallower_ply = table.probe(key, 2).expect("entry should be present");
    assert_eq!(same_ply.score, VALUE_MATE - 7);
    assert_eq!(shallower_ply.score, VALUE_MATE - 4);
    assert_eq!(shallower_ply.best_move, best);
}
