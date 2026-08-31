use neyrang::{
    chess::{Move, MoveFlag, Square},
    search::{
        VALUE_MATE,
        tt::{Bound, TranspositionTable},
    },
};

const MEBIBYTE: usize = 1024 * 1024;

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

#[test]
fn shared_table_packs_entries_inside_the_total_hash_budget() {
    let table = TranspositionTable::new_shared(1);

    assert!(table.is_shared());
    assert!(table.allocated_bytes() <= MEBIBYTE);
    assert!(table.allocated_bytes() >= MEBIBYTE / 2);
}

#[test]
fn shared_table_round_trips_fields_and_rejects_signature_collisions() {
    let mut table = TranspositionTable::new_shared(1);
    let best = Move::new(Square::E2, Square::E4, MoveFlag::DoublePawnPush);
    let key = 0x1234_5678_0000_0042;
    let same_index_different_signature = 0x4321_5678_0000_0042;

    table.store(key, 17, -321, Bound::Lower, best, 3);

    assert_eq!(
        table.probe(key, 3).expect("shared entry should exist"),
        neyrang::search::tt::TtData {
            best_move: best,
            score: -321,
            depth: 17,
            bound: Bound::Lower,
        }
    );
    assert!(table.probe(same_index_different_signature, 3).is_none());
}

#[test]
fn shared_table_preserves_mate_normalization_generation_and_clear() {
    let mut table = TranspositionTable::new_shared(1);
    let best = Move::new(Square::G6, Square::G7, MoveFlag::Quiet);
    let key = 0xBEEF_0000_0000_0088;

    table.store(key, 8, VALUE_MATE - 7, Bound::Exact, best, 5);
    assert_eq!(
        table.probe(key, 2).expect("entry should exist").score,
        VALUE_MATE - 4
    );

    table.new_search();
    assert!(
        table.probe(key, 5).is_some(),
        "aging must not erase an entry"
    );
    table.clear();
    assert!(table.probe(key, 5).is_none());
    assert_eq!(table.hashfull(), 0);
}

#[test]
fn shared_table_keeps_deep_current_entries_and_replaces_aged_entries() {
    let mut table = TranspositionTable::new_shared(1);
    let old_key = 0x1111_0000_0000_0020;
    let colliding_key = 0x2222_0000_0000_0020;
    let old_move = Move::new(Square::A2, Square::A4, MoveFlag::DoublePawnPush);
    let new_move = Move::new(Square::H2, Square::H4, MoveFlag::DoublePawnPush);

    table.store(old_key, 20, 80, Bound::Lower, old_move, 0);
    table.store(colliding_key, 2, 20, Bound::Upper, new_move, 0);
    assert!(table.probe(old_key, 0).is_some());
    assert!(table.probe(colliding_key, 0).is_none());

    table.new_search();
    table.store(colliding_key, 2, 20, Bound::Upper, new_move, 0);
    assert!(table.probe(old_key, 0).is_none());
    assert_eq!(
        table
            .probe(colliding_key, 0)
            .expect("aged entry should be replaced")
            .best_move,
        new_move
    );
}

#[test]
fn shared_handles_publish_coherent_entries_concurrently() {
    let mut owner = TranspositionTable::new_shared(1);
    owner.new_search();

    std::thread::scope(|scope| {
        let handles = (0_u64..8)
            .map(|worker| {
                let mut table = owner.shared_handle();
                scope.spawn(move || {
                    for slot in 0_u64..1_000 {
                        let index = worker * 1_024 + slot;
                        let key = ((worker + 1) << 48) | index;
                        let score = worker as i32 * 1_000 + slot as i32;
                        let best = Move::new(
                            Square::from_index(worker as u8).expect("worker square"),
                            Square::from_index((slot % 64) as u8).expect("slot square"),
                            MoveFlag::Quiet,
                        );
                        table.store(key, 12, score, Bound::Exact, best, 0);
                        let data = table.probe(key, 0).expect("just-stored entry");
                        assert_eq!(
                            (data.best_move, data.score, data.depth, data.bound),
                            (best, score, 12, Bound::Exact)
                        );
                    }
                })
            })
            .collect::<Vec<_>>();

        for handle in handles {
            handle.join().expect("shared TT worker must not panic");
        }
    });

    for worker in 0_u64..8 {
        let slot = 999_u64;
        let key = ((worker + 1) << 48) | (worker * 1_024 + slot);
        assert_eq!(
            owner.probe(key, 0).expect("published entry").score,
            worker as i32 * 1_000 + slot as i32
        );
    }
}
