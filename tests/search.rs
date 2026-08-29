use std::sync::atomic::{AtomicBool, Ordering};

use neyrang::{
    chess::Position,
    search::{SearchLimits, Searcher, VALUE_MATE},
};

#[test]
fn iterative_deepening_finds_a_mate_in_one() {
    let mut position =
        Position::from_fen("7k/8/5KQ1/8/8/8/8/8 w - - 0 1").expect("mate fixture is valid");
    let stop = AtomicBool::new(false);
    let mut searcher = Searcher::new(&stop);
    let game_hashes = [position.hash()];

    let result = searcher.search(&mut position, &SearchLimits::depth(3), &game_hashes, |_| {});

    assert!(result.score >= VALUE_MATE - 2);
    let best = result.best_move.expect("a mating move must be returned");
    let undo = position.make_move(best);
    assert!(position.is_in_check(position.side_to_move()));
    assert!(position.legal_moves().is_empty());
    position.unmake_move(best, undo);
}

#[test]
fn a_pre_signaled_stop_is_not_lost_during_search_startup() {
    let mut position = Position::startpos();
    let stop = AtomicBool::new(false);
    stop.store(true, Ordering::Relaxed);
    let mut searcher = Searcher::new(&stop);
    let hashes = [position.hash()];

    let result = searcher.search(&mut position, &SearchLimits::depth(4), &hashes, |_| {});

    assert!(result.stopped);
    assert!(result.nodes <= 1);
    assert!(result.best_move.is_some());
}

#[test]
fn stalemate_and_fifty_move_rule_return_draw_scores() {
    let stop = AtomicBool::new(false);
    let mut stalemate =
        Position::from_fen("7k/5Q2/6K1/8/8/8/8/8 b - - 0 1").expect("stalemate fixture is valid");
    let stalemate_hashes = [stalemate.hash()];
    let mut searcher = Searcher::new(&stop);
    let result = searcher.search(
        &mut stalemate,
        &SearchLimits::depth(2),
        &stalemate_hashes,
        |_| {},
    );
    assert_eq!(result.score, 0);
    assert!(result.best_move.is_none());

    let mut fifty_move = Position::from_fen("7k/8/8/8/8/8/6R1/6K1 w - - 100 51")
        .expect("fifty-move fixture is valid");
    let fifty_hashes = [fifty_move.hash()];
    let mut searcher = Searcher::new(&stop);
    let result = searcher.search(
        &mut fifty_move,
        &SearchLimits::depth(2),
        &fifty_hashes,
        |_| {},
    );
    assert_eq!(result.score, 0);
    assert!(result.best_move.is_some());
}

#[cfg(feature = "stats")]
#[test]
fn pvs_uses_zero_window_searches_after_the_first_move() {
    let mut position = Position::startpos();
    let hashes = [position.hash()];
    let stop = AtomicBool::new(false);
    let mut searcher = Searcher::new(&stop);

    let result = searcher.search(&mut position, &SearchLimits::depth(4), &hashes, |_| {});

    assert!(result.statistics.pvs_zero_window_searches > 0);
    assert!(result.statistics.aspiration_searches > 0);
}

#[cfg(feature = "stats")]
#[test]
fn capture_ordering_reports_see_and_cutoff_statistics() {
    let mut position = Position::from_fen("6k1/8/5p2/3qp3/2P1Q3/8/8/6K1 w - - 0 1")
        .expect("ordering statistics fixture is valid");
    let hashes = [position.hash()];
    let stop = AtomicBool::new(false);
    let mut searcher = Searcher::new(&stop);

    let result = searcher.search(&mut position, &SearchLimits::depth(4), &hashes, |_| {});

    assert!(result.statistics.see_calls > 0);
    assert!(result.statistics.good_captures > 0);
    assert!(result.statistics.bad_captures > 0);
    assert!(result.statistics.tt_move_searches > 0);
    assert!(
        result.statistics.capture_beta_cutoffs + result.statistics.quiet_beta_cutoffs
            <= result.statistics.beta_cutoffs
    );
}

#[cfg(feature = "stats")]
#[test]
fn quiescence_reports_pruned_losing_captures() {
    let mut position = Position::from_fen("6k1/8/5p2/4p3/4Q3/8/8/6K1 w - - 0 1")
        .expect("quiescence SEE fixture is valid");
    let hashes = [position.hash()];
    let stop = AtomicBool::new(false);
    let mut searcher = Searcher::new(&stop);

    let result = searcher.search(&mut position, &SearchLimits::depth(3), &hashes, |_| {});

    assert!(result.statistics.see_prunes > 0);
}

#[cfg(feature = "stats")]
#[test]
fn lmr_reduces_late_quiet_moves() {
    let mut position = Position::startpos();
    let hashes = [position.hash()];
    let stop = AtomicBool::new(false);
    let mut searcher = Searcher::new(&stop);

    let result = searcher.search(&mut position, &SearchLimits::depth(6), &hashes, |_| {});

    assert!(result.statistics.lmr_reductions > 0);
    assert!(result.statistics.lmr_researches > 0);
    assert!(result.statistics.lmr_researches <= result.statistics.lmr_reductions);
}
