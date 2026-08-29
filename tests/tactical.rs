use std::sync::atomic::AtomicBool;

use neyrang::{
    chess::Position,
    search::{SearchLimits, SearchResult, Searcher, VALUE_MATE},
};

fn search(fen: &str, depth: u8) -> (Position, SearchResult) {
    let mut position = Position::from_fen(fen).expect("tactical fixture must be valid");
    let hashes = [position.hash()];
    let stop = AtomicBool::new(false);
    let mut searcher = Searcher::new(&stop);
    let result = searcher.search(&mut position, &SearchLimits::depth(depth), &hashes, |_| {});
    (position, result)
}

fn assert_best_in(result: &SearchResult, expected: &[&str]) {
    let best = result
        .best_move
        .expect("a tactical fixture must have a legal best move")
        .to_string();
    assert!(
        expected.contains(&best.as_str()),
        "unexpected best move {best}; expected one of {expected:?}"
    );
}

fn assert_legal_pv(mut position: Position, result: &SearchResult) {
    for &mv in &result.pv {
        assert!(
            position.legal_moves().iter().any(|&legal| legal == mv),
            "PV move {mv} must be legal in its position"
        );
        let _ = position.make_move(mv);
    }
}

#[test]
fn finds_mate_in_one() {
    let (position, result) = search("7k/8/5KQ1/8/8/8/8/8 w - - 0 1", 3);

    assert!(result.score >= VALUE_MATE - 2);
    assert_legal_pv(position, &result);
}

#[test]
fn finds_tablebase_mate_in_two() {
    let (position, result) = search("8/7k/5K2/4Q3/8/8/8/8 w - - 0 1", 4);

    assert!(result.score >= VALUE_MATE - 4);
    assert_best_in(&result, &["e5g3", "e5g5", "e5c7", "e5b8", "e5e8"]);
    assert_legal_pv(position, &result);
}

#[test]
fn finds_tablebase_mate_in_three() {
    let (position, result) = search("8/7k/8/5K2/8/8/8/3Q4 w - - 0 1", 6);

    assert!(result.score >= VALUE_MATE - 6);
    assert_best_in(&result, &["d1g1", "d1d4", "d1g4", "d1d7", "f5f6"]);
    assert_legal_pv(position, &result);
}

#[test]
fn takes_a_hanging_queen() {
    let (position, result) = search("6k1/8/8/3q4/4P3/8/8/6K1 w - - 0 1", 3);

    assert_best_in(&result, &["e4d5"]);
    assert_legal_pv(position, &result);
}

#[test]
fn finds_the_forced_recapture() {
    let (position, result) = search("7k/8/8/8/8/8/3PrP2/3NKN2 w - - 0 1", 2);

    assert_best_in(&result, &["e1e2"]);
    assert_legal_pv(position, &result);
}

#[test]
fn preserves_a_quiet_mating_move() {
    let (position, result) = search("8/7k/5K2/4Q3/8/8/8/8 w - - 0 1", 4);

    let best = result.best_move.expect("the mate must have a best move");
    assert!(!best.is_capture());
    assert!(!best.is_promotion());
    assert!(result.score >= VALUE_MATE - 4);
    assert_legal_pv(position, &result);
}

#[test]
fn promotes_in_a_simple_race() {
    let (position, result) = search("7k/P7/8/8/8/8/8/K7 w - - 0 1", 2);

    assert_best_in(&result, &["a7a8q"]);
    assert_legal_pv(position, &result);
}

#[test]
fn underpromotes_to_avoid_stalemate() {
    let (position, result) = search("8/k1P5/8/1K6/8/8/8/8 w - - 0 1", 1);

    assert_best_in(&result, &["c7c8r"]);
    assert_legal_pv(position, &result);
}

#[test]
fn rejects_a_queen_promotion_stalemate_trap() {
    let (mut position, result) = search("8/k1P5/8/1K6/8/8/8/8 w - - 0 1", 3);
    let stalemating_move = position
        .find_legal_move("c7c8q")
        .expect("queen promotion must be legal");
    let undo = position.make_move(stalemating_move);
    assert!(!position.is_in_check(position.side_to_move()));
    assert!(position.legal_moves().is_empty());
    position.unmake_move(stalemating_move, undo);

    assert_ne!(result.best_move, Some(stalemating_move));
    assert_best_in(&result, &["c7c8r", "b5c6"]);
    assert_legal_pv(position, &result);
}

#[test]
fn keeps_the_opposition_in_a_zugzwang_like_endgame() {
    let (position, result) = search("4k3/8/8/4K3/4P3/8/8/8 w - - 0 1", 10);

    assert_best_in(&result, &["e5e6", "e5d6", "e5f6"]);
    assert_legal_pv(position, &result);
}

#[test]
fn finds_the_only_defensive_quiet_block() {
    let (position, result) = search("rr6/8/8/8/8/2k5/7R/K7 w - - 0 1", 3);

    assert_best_in(&result, &["h2a2"]);
    assert_legal_pv(position, &result);
}
