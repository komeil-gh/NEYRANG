use std::sync::atomic::AtomicBool;

use super::{SearchContext, SearchStatistics, Searcher};
use crate::{chess::Position, eval, search::SearchLimits};

fn search_window(
    mut position: Position,
    depth: i32,
    beta: i32,
    context: SearchContext,
) -> (i32, SearchStatistics) {
    let original = position.clone();
    let stop = AtomicBool::new(false);
    let mut searcher = Searcher::new(&stop);
    let hashes = [position.repetition_hash()];
    searcher.reset(&mut position, &SearchLimits::depth(depth as u8), &hashes);

    let score = searcher.negamax(&mut position, depth, 1, beta - 1, beta, context);
    let pv = searcher.pv_line(1);
    let statistics = searcher.statistics;

    assert_eq!(position, original);
    let mut replay = original;
    for mv in pv {
        assert!(
            replay.legal_moves().iter().any(|&legal| legal == mv),
            "synthetic null state must never leak into the PV"
        );
        replay.make_move(mv);
    }

    (score, statistics)
}

#[test]
fn zugzwang_prone_material_is_guarded_and_matches_the_reference_search() {
    let fixtures = [
        "4k3/8/8/4K3/4P3/8/8/8 w - - 0 1",
        "8/8/8/2k5/2p5/2K5/2P5/8 w - - 0 1",
        "8/8/3k4/3p4/3P4/3K4/8/8 w - - 0 1",
        "4k3/8/8/8/8/8/3B4/4K3 w - - 0 1",
        "4k3/8/8/8/8/8/3N4/4K3 w - - 0 1",
    ];

    for fen in fixtures {
        let position = Position::from_fen(fen).expect("zugzwang fixture must be valid");
        let (reference, reference_stats) =
            search_window(position.clone(), 6, 0, SearchContext::without_null(None));
        let (candidate, candidate_stats) =
            search_window(position, 6, 0, SearchContext::normal(None));

        assert_eq!(candidate, reference, "FEN: {fen}");
        assert_eq!(reference_stats.null_move_attempts, 0, "FEN: {fen}");
        assert_eq!(candidate_stats.null_move_attempts, 0, "FEN: {fen}");
        assert_eq!(candidate_stats.null_move_cutoffs, 0, "FEN: {fen}");
    }
}

#[test]
fn sparse_major_piece_windows_keep_the_reference_bound_classification() {
    for fen in [
        "8/8/8/4k3/8/3K4/3P4/3R4 w - - 0 1",
        "8/8/8/4k3/8/3K4/3P4/3Q4 w - - 0 1",
    ] {
        let position = Position::from_fen(fen).expect("major-piece fixture must be valid");
        let beta = eval::evaluate(&position) - 50;
        let (reference, _) =
            search_window(position.clone(), 6, beta, SearchContext::without_null(None));
        let (candidate, candidate_stats) =
            search_window(position, 6, beta, SearchContext::normal(None));

        assert!(candidate_stats.null_move_attempts > 0, "FEN: {fen}");
        assert_eq!(candidate >= beta, reference >= beta, "FEN: {fen}");
        assert_eq!(candidate_stats.null_move_verifications, 0, "FEN: {fen}");
    }
}
