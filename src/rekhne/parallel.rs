use std::{
    sync::atomic::{AtomicBool, AtomicU64, Ordering},
    time::Instant,
};

use crate::{
    chess::{Move, Position},
    sanj,
};

use super::{
    SearchInfo, SearchLimits, SearchResult, SearchStatistics, Searcher, driver::SearchProgress,
    tt::TranspositionTable,
};

pub(crate) struct ParallelOptions {
    threads: usize,
    evaluator: sanj::Evaluator,
}

impl ParallelOptions {
    pub(crate) const fn new(threads: usize, evaluator: sanj::Evaluator) -> Self {
        Self { threads, evaluator }
    }
}

#[cfg(test)]
pub(crate) fn search_parallel<F>(
    position: &Position,
    limits: &SearchLimits,
    game_hashes: &[u64],
    threads: usize,
    stop: &AtomicBool,
    table: TranspositionTable,
    on_info: F,
) -> (SearchResult, TranspositionTable)
where
    F: FnMut(&SearchInfo),
{
    search_parallel_with_evaluator(
        position,
        limits,
        game_hashes,
        stop,
        table,
        ParallelOptions::new(threads, sanj::Evaluator::classical()),
        on_info,
    )
}

pub(crate) fn search_parallel_with_evaluator<F>(
    position: &Position,
    limits: &SearchLimits,
    game_hashes: &[u64],
    stop: &AtomicBool,
    mut table: TranspositionTable,
    options: ParallelOptions,
    mut on_info: F,
) -> (SearchResult, TranspositionTable)
where
    F: FnMut(&SearchInfo),
{
    let ParallelOptions { threads, evaluator } = options;
    assert!(threads > 1, "parallel search requires at least two workers");
    assert!(table.is_shared(), "parallel search requires a shared TT");

    table.new_search();
    let started = Instant::now();
    let global_nodes = limits.nodes.map(|_| AtomicU64::new(0));
    let progress = (0..threads)
        .map(|_| SearchProgress::default())
        .collect::<Vec<_>>();
    let mut root = position.clone();
    let root_moves = root.legal_moves();

    let (mut results, table) = std::thread::scope(|scope| {
        let mut helpers = Vec::with_capacity(threads - 1);
        for (worker, worker_progress) in progress.iter().enumerate().skip(1) {
            let mut worker_position = position.clone();
            let worker_table = table.shared_handle();
            let worker_global_nodes = global_nodes.as_ref();
            let worker_evaluator = evaluator.clone();
            let preferred = if root_moves.is_empty() {
                None
            } else {
                Some(root_moves.as_slice()[worker % root_moves.len()])
            };
            helpers.push((
                worker,
                scope.spawn(move || {
                    let mut searcher = Searcher::with_parallel_context_and_evaluator(
                        stop,
                        worker_table,
                        worker_global_nodes,
                        worker_progress,
                        worker_evaluator,
                    );
                    searcher.search_prepared(
                        &mut worker_position,
                        limits,
                        game_hashes,
                        started,
                        preferred,
                        |_| {},
                    )
                }),
            ));
        }

        let mut main_position = position.clone();
        let mut main_searcher = Searcher::with_parallel_context_and_evaluator(
            stop,
            table,
            global_nodes.as_ref(),
            &progress[0],
            evaluator,
        );
        let main_result = main_searcher.search_prepared(
            &mut main_position,
            limits,
            game_hashes,
            started,
            None,
            |info| {
                let (nodes, qnodes) = aggregate_progress(&progress);
                let mut aggregate = info.clone();
                aggregate.nodes = nodes;
                aggregate.qnodes = qnodes;
                on_info(&aggregate);
            },
        );
        let table = main_searcher.into_table();
        let mut results = vec![(0, main_result)];
        for (worker, handle) in helpers {
            let result = handle.join().expect("Lazy SMP helper thread panicked");
            results.push((worker, result));
        }
        (results, table)
    });

    results.sort_by_key(|(worker, _)| *worker);
    let selected = select_result(&results);
    let mut result = results[selected].1.clone();
    result.nodes = results.iter().map(|(_, result)| result.nodes).sum();
    result.qnodes = results.iter().map(|(_, result)| result.qnodes).sum();
    result.seldepth = results
        .iter()
        .map(|(_, result)| result.seldepth)
        .max()
        .unwrap_or_default();
    result.elapsed = started.elapsed();
    result.stopped = results.iter().any(|(_, result)| result.stopped);
    result.hashfull = table.hashfull();
    result.statistics = aggregate_statistics(&results);

    if let Some(global_nodes) = &global_nodes {
        debug_assert_eq!(result.nodes, global_nodes.load(Ordering::Relaxed));
    }

    on_info(&SearchInfo {
        depth: result.depth,
        seldepth: result.seldepth,
        score: result.score,
        nodes: result.nodes,
        qnodes: result.qnodes,
        elapsed: result.elapsed,
        pv: result.pv.clone(),
        hashfull: result.hashfull,
    });
    (result, table)
}

fn aggregate_progress(progress: &[SearchProgress]) -> (u64, u64) {
    progress
        .iter()
        .map(SearchProgress::snapshot)
        .fold((0, 0), |(nodes, qnodes), current| {
            (
                nodes.saturating_add(current.0),
                qnodes.saturating_add(current.1),
            )
        })
}

fn aggregate_statistics(results: &[(usize, SearchResult)]) -> SearchStatistics {
    let mut statistics = SearchStatistics::default();
    for (_, result) in results {
        statistics.accumulate(result.statistics);
    }
    statistics
}

fn select_result(results: &[(usize, SearchResult)]) -> usize {
    let has_completed_result = results
        .iter()
        .any(|(_, result)| result.depth > 0 && result.best_move.is_some());
    let eligible = results
        .iter()
        .enumerate()
        .filter_map(|(index, (_, result))| {
            (!has_completed_result || result.depth > 0)
                .then_some(result.best_move)
                .flatten()
                .map(|mv| (index, mv))
        })
        .collect::<Vec<_>>();
    if eligible.is_empty() {
        return 0;
    }

    let minimum_score = eligible
        .iter()
        .map(|(index, _)| results[*index].1.score)
        .min()
        .expect("eligible results are non-empty");
    let mut groups = Vec::<VoteGroup>::new();
    for (index, mv) in &eligible {
        let (worker, result) = &results[*index];
        let vote = i64::from(result.score) - i64::from(minimum_score) + 14;
        if let Some(group) = groups.iter_mut().find(|group| group.mv == *mv) {
            group.votes += vote;
            group.depth = group.depth.max(result.depth);
            group.pv_len = group.pv_len.max(result.pv.len());
            group.has_main |= *worker == 0;
        } else {
            groups.push(VoteGroup {
                mv: *mv,
                votes: vote,
                depth: result.depth,
                pv_len: result.pv.len(),
                has_main: *worker == 0,
            });
        }
    }
    let winning_group = groups
        .iter()
        .max_by_key(|group| (group.votes, group.depth, group.pv_len, group.has_main))
        .expect("at least one vote group");

    eligible
        .into_iter()
        .filter(|(_, mv)| *mv == winning_group.mv)
        .max_by_key(|(index, _)| {
            let (worker, result) = &results[*index];
            (
                result.depth,
                result.score,
                result.pv.len(),
                usize::MAX - *worker,
            )
        })
        .map(|(index, _)| index)
        .expect("winning group has a representative")
}

struct VoteGroup {
    mv: Move,
    votes: i64,
    depth: u8,
    pv_len: usize,
    has_main: bool,
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicBool, Ordering};

    use crate::{
        chess::Position,
        rekhne::{SearchLimits, tt::TranspositionTable},
    };

    use super::{search_parallel, select_result};
    use crate::rekhne::{SearchResult, SearchStatistics};

    #[test]
    fn depth_search_returns_a_legal_pv_and_exact_final_aggregate() {
        let mut position = Position::startpos();
        let hashes = [position.repetition_hash()];
        let stop = AtomicBool::new(false);
        let mut infos = Vec::new();

        let (result, table) = search_parallel(
            &position,
            &SearchLimits::depth(4),
            &hashes,
            2,
            &stop,
            TranspositionTable::new_shared(16),
            |info| infos.push(info.clone()),
        );

        assert!(table.is_shared());
        assert_eq!(result.depth, 4);
        assert!(result.nodes > 0);
        assert!(result.best_move.is_some());
        assert!(infos.len() <= 5, "helpers must not emit info: {infos:?}");
        assert_eq!(
            infos.last().expect("final aggregate info").nodes,
            result.nodes
        );
        assert_eq!(
            infos.last().expect("final aggregate info").qnodes,
            result.qnodes
        );
        assert!(
            result
                .pv
                .iter()
                .try_fold(position.clone(), |mut current, &mv| {
                    current
                        .legal_moves()
                        .iter()
                        .any(|&legal| legal == mv)
                        .then(|| {
                            current.make_move(mv);
                            current
                        })
                })
                .is_some(),
            "selected PV must be legal"
        );
    }

    #[test]
    fn global_node_limit_is_aggregate_and_has_only_bounded_race_overshoot() {
        let mut position = Position::startpos();
        let hashes = [position.repetition_hash()];
        let stop = AtomicBool::new(false);
        let limit = 10_000;

        let (result, _) = search_parallel(
            &position,
            &SearchLimits::nodes(limit),
            &hashes,
            4,
            &stop,
            TranspositionTable::new_shared(16),
            |_| {},
        );

        assert!(result.stopped);
        assert!(result.nodes >= limit);
        assert!(
            result.nodes <= limit + 3,
            "aggregate nodes: {}",
            result.nodes
        );
    }

    #[test]
    fn pre_signalled_stop_returns_one_legal_fallback_per_search_not_per_output() {
        let mut position = Position::startpos();
        let hashes = [position.repetition_hash()];
        let stop = AtomicBool::new(true);
        let mut info_count = 0;

        let (result, _) = search_parallel(
            &position,
            &SearchLimits::depth(12),
            &hashes,
            4,
            &stop,
            TranspositionTable::new_shared(16),
            |_| info_count += 1,
        );

        assert!(result.stopped);
        assert!(result.best_move.is_some());
        assert!(result.nodes <= 4);
        assert_eq!(info_count, 1, "only the final aggregate line is emitted");
    }

    #[test]
    fn terminal_root_is_reported_without_a_move() {
        let mut position =
            Position::from_fen("7k/5Q2/6K1/8/8/8/8/8 b - - 0 1").expect("stalemate fixture");
        let hashes = [position.repetition_hash()];
        let stop = AtomicBool::new(false);

        let (result, _) = search_parallel(
            &position,
            &SearchLimits::depth(4),
            &hashes,
            2,
            &stop,
            TranspositionTable::new_shared(16),
            |_| {},
        );

        assert!(result.best_move.is_none());
        assert_eq!(result.score, 0);
        assert_eq!(result.nodes, 0);
    }

    #[test]
    fn an_external_stop_reaches_all_workers() {
        let mut position = Position::startpos();
        let hashes = [position.repetition_hash()];
        let stop = AtomicBool::new(false);

        std::thread::scope(|scope| {
            let stopper = scope.spawn(|| {
                std::thread::sleep(std::time::Duration::from_millis(20));
                stop.store(true, Ordering::Relaxed);
            });
            let (result, _) = search_parallel(
                &position,
                &SearchLimits::infinite(),
                &hashes,
                4,
                &stop,
                TranspositionTable::new_shared(16),
                |_| {},
            );
            stopper.join().expect("stopper thread");
            assert!(result.stopped);
            assert!(result.best_move.is_some());
        });
    }

    #[test]
    fn final_vote_uses_group_score_then_depth_and_main_ties() {
        let mut position = Position::startpos();
        let legal = position.legal_moves();
        let first = legal.as_slice()[0];
        let second = legal.as_slice()[1];
        let results = vec![
            (0, result(first, 0, 5, 2)),
            (1, result(first, 0, 5, 2)),
            (2, result(second, 14, 6, 3)),
        ];

        let selected = select_result(&results);

        assert_eq!(results[selected].1.best_move, Some(second));

        let tied_representatives = vec![(0, result(first, 20, 7, 4)), (1, result(first, 20, 7, 4))];
        assert_eq!(select_result(&tied_representatives), 0);

        let incomplete_helper = vec![
            (0, result(first, 0, 5, 2)),
            (1, result(second, 10_000, 0, 1)),
        ];
        assert_eq!(select_result(&incomplete_helper), 0);
    }

    fn result(best_move: crate::chess::Move, score: i32, depth: u8, pv_len: usize) -> SearchResult {
        SearchResult {
            best_move: Some(best_move),
            score,
            depth,
            seldepth: depth,
            nodes: 1,
            qnodes: 0,
            elapsed: std::time::Duration::from_millis(1),
            pv: vec![best_move; pv_len],
            stopped: false,
            hashfull: 0,
            statistics: SearchStatistics::default(),
        }
    }
}
