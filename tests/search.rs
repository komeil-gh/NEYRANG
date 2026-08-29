use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

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
fn an_expired_hard_limit_returns_the_legal_fallback_immediately() {
    let mut position = Position::startpos();
    let stop = AtomicBool::new(false);
    let mut searcher = Searcher::new(&stop);
    let hashes = [position.hash()];
    let limits = SearchLimits {
        depth: Some(12),
        hard_time: Some(Duration::ZERO),
        ..SearchLimits::default()
    };

    let result = searcher.search(&mut position, &limits, &hashes, |_| {});

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

#[test]
fn principal_variation_stops_at_threefold_after_uncapturable_en_passant() {
    const MOVES: &str = "e2c4 c8d7 c1d2 g8f6 f1d3 a5b4 c3e4 b4b2 d2c3 b2b6 \
        e4f6 g7f6 c3f6 h8g8 e1g1 h7h5 d3e2 g8g6 f6c3 g6g5 c3f6 g5f5 c4h4 b6b4 \
        c2c4 f8e7 a2a3 b4c5 f6e7 c5e7 h4g3 f5g5 g3e3 a7a5 h2h4 g5g8 e3e4 g8h8 \
        f1d1 e6e5 a1b1 f7f5 e4d3 a5a4 c4c5 e5e4 d3g3 e8f8 d1d6 h8g8 g3h3 g8g7 \
        e2h5 f5f4 h3h2 e7e5 h5e2 a8e8 h4h5 e4e3 b1f1 g7f7 e2c4 e3e2 c4e2 e5e2 \
        d6d7 f7d7 h2f4 f8g8 f4a4 d7d5 g2g4 e8e4 a4a8 e4e8 a8a4 e8e4 a4a8 e4e8";

    let mut position =
        Position::from_fen("r1b1kbnr/p4ppp/2p1p3/q7/8/2N5/PPP1QPPP/R1B1KB1R w KQkq - 2 9")
            .expect("fixture is valid");
    let mut hashes = vec![position.repetition_hash()];
    for notation in MOVES.split_whitespace() {
        let mv = position
            .find_legal_move(notation)
            .unwrap_or_else(|| panic!("fixture move {notation} must be legal"));
        position.make_move(mv);
        hashes.push(position.repetition_hash());
    }

    let stop = AtomicBool::new(false);
    let mut searcher = Searcher::new(&stop);
    let result = searcher.search(&mut position, &SearchLimits::depth(1), &hashes, |_| {});

    assert_eq!(
        result.best_move.map(|mv| mv.to_string()).as_deref(),
        Some("a8a4")
    );
    assert_eq!(
        result
            .pv
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>(),
        ["a8a4"]
    );
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
    assert!(result.statistics.see_ge_calls > 0);
    assert!(result.statistics.total_see_exchange_steps() > 0);
    assert!(result.statistics.good_captures > 0);
    assert!(result.statistics.bad_captures > 0);
    assert!(result.statistics.tt_move_searches > 0);
    assert!(result.statistics.move_generation_calls > 0);
    assert!(result.statistics.moves_generated >= result.statistics.moves_scored);
    assert!(result.statistics.ordering_calls > 0);
    assert_eq!(result.statistics.full_sorts, 0);
    assert!(result.statistics.moves_scored >= result.statistics.scored_moves_searched);
    assert!(result.statistics.scored_moves_searched <= result.statistics.moves_searched);
    assert!(result.statistics.moves_scored_unused() > 0);
    assert!(
        result.statistics.see_calls + result.statistics.see_ge_calls
            >= result.statistics.see_scored_moves_searched
    );
    assert!(result.statistics.see_scored_moves_unused() > 0);
    assert!(result.statistics.tactical_candidates_untested > 0);
    assert!(result.statistics.picker_tt_stage_visits > 0);
    assert!(result.statistics.picker_good_tactical_stage_visits > 0);
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
