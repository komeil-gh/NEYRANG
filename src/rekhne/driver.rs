//! Iterative search driver and recursive alpha-beta implementation.

use std::{
    sync::atomic::{AtomicBool, AtomicU64, Ordering},
    time::{Duration, Instant},
};

#[cfg(feature = "nnue")]
use crate::sanj::nnue::AccumulatorPair;
use crate::{
    chess::{Move, MoveList, PieceType, Position},
    sanj,
    shegerd::{history::HistoryTable, ordering},
};

use super::{
    SearchLimits,
    tt::{Bound, TranspositionTable},
};

pub const MAX_PLY: usize = 128;
pub const VALUE_DRAW: i32 = 0;
pub const VALUE_MATE: i32 = 30_000;
pub const VALUE_INFINITE: i32 = 32_000;
const NULL_MOVE_MIN_DEPTH: i32 = 4;
const NULL_MOVE_REDUCTION: i32 = 2;

#[derive(Clone, Copy)]
struct SearchContext {
    preferred: Option<Move>,
    null_allowed: bool,
    in_null_subtree: bool,
}

impl SearchContext {
    const fn normal(preferred: Option<Move>) -> Self {
        Self {
            preferred,
            null_allowed: true,
            in_null_subtree: false,
        }
    }

    #[cfg(all(test, feature = "stats"))]
    const fn without_null(preferred: Option<Move>) -> Self {
        Self {
            preferred,
            null_allowed: false,
            in_null_subtree: false,
        }
    }

    const fn after_move(self) -> Self {
        Self {
            preferred: None,
            null_allowed: self.null_allowed,
            in_null_subtree: self.in_null_subtree,
        }
    }

    const fn after_null(self) -> Self {
        Self {
            preferred: None,
            null_allowed: false,
            in_null_subtree: true,
        }
    }
}

#[derive(Clone, Copy)]
struct SearchSetup {
    started: Option<Instant>,
    root_preferred: Option<Move>,
    advance_generation: bool,
}

impl SearchSetup {
    const fn normal() -> Self {
        Self {
            started: None,
            root_preferred: None,
            advance_generation: true,
        }
    }

    const fn prepared(started: Instant, root_preferred: Option<Move>) -> Self {
        Self {
            started: Some(started),
            root_preferred,
            advance_generation: false,
        }
    }
}

#[derive(Clone, Debug)]
pub struct SearchInfo {
    pub depth: u8,
    pub seldepth: u8,
    pub score: i32,
    pub nodes: u64,
    pub qnodes: u64,
    pub elapsed: Duration,
    pub pv: Vec<Move>,
    pub hashfull: u16,
}

impl SearchInfo {
    pub fn nps(&self) -> u64 {
        let micros = self.elapsed.as_micros().max(1) as u64;
        self.nodes.saturating_mul(1_000_000) / micros
    }
}

#[derive(Clone, Debug)]
pub struct SearchResult {
    pub best_move: Option<Move>,
    pub score: i32,
    pub depth: u8,
    pub seldepth: u8,
    pub nodes: u64,
    pub qnodes: u64,
    pub elapsed: Duration,
    pub pv: Vec<Move>,
    pub stopped: bool,
    pub hashfull: u16,
    pub statistics: SearchStatistics,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SearchStatistics {
    pub tt_hits: u64,
    pub tt_cutoffs: u64,
    pub beta_cutoffs: u64,
    pub first_move_beta_cutoffs: u64,
    pub capture_beta_cutoffs: u64,
    pub quiet_beta_cutoffs: u64,
    pub tt_move_searches: u64,
    pub see_calls: u64,
    pub good_captures: u64,
    pub bad_captures: u64,
    pub pvs_zero_window_searches: u64,
    pub pvs_researches: u64,
    pub aspiration_searches: u64,
    pub see_prunes: u64,
    pub futility_prunes: u64,
    pub null_move_attempts: u64,
    pub null_move_fail_highs: u64,
    pub null_move_cutoffs: u64,
    pub null_move_verifications: u64,
    pub lmr_reductions: u64,
    pub lmr_researches: u64,
    #[cfg(feature = "stats")]
    pub move_generation_calls: u64,
    #[cfg(feature = "stats")]
    pub moves_generated: u64,
    #[cfg(feature = "stats")]
    pub ordering_calls: u64,
    #[cfg(feature = "stats")]
    pub moves_scored: u64,
    #[cfg(feature = "stats")]
    pub full_sorts: u64,
    #[cfg(feature = "stats")]
    pub moves_searched: u64,
    #[cfg(feature = "stats")]
    pub scored_moves_searched: u64,
    #[cfg(feature = "stats")]
    pub tactical_moves_searched: u64,
    #[cfg(feature = "stats")]
    pub see_scored_moves_searched: u64,
    #[cfg(feature = "stats")]
    pub picker_tt_stage_visits: u64,
    #[cfg(feature = "stats")]
    pub picker_good_tactical_stage_visits: u64,
    #[cfg(feature = "stats")]
    pub picker_killer_stage_visits: u64,
    #[cfg(feature = "stats")]
    pub picker_quiet_stage_visits: u64,
    #[cfg(feature = "stats")]
    pub picker_bad_tactical_stage_visits: u64,
}

impl SearchStatistics {
    #[cfg(feature = "stats")]
    pub fn moves_scored_unused(self) -> u64 {
        self.moves_scored.saturating_sub(self.scored_moves_searched)
    }

    #[cfg(feature = "stats")]
    pub fn see_scored_moves_unused(self) -> u64 {
        self.see_calls
            .saturating_sub(self.see_scored_moves_searched)
    }

    pub(crate) fn accumulate(&mut self, other: Self) {
        self.tt_hits += other.tt_hits;
        self.tt_cutoffs += other.tt_cutoffs;
        self.beta_cutoffs += other.beta_cutoffs;
        self.first_move_beta_cutoffs += other.first_move_beta_cutoffs;
        self.capture_beta_cutoffs += other.capture_beta_cutoffs;
        self.quiet_beta_cutoffs += other.quiet_beta_cutoffs;
        self.tt_move_searches += other.tt_move_searches;
        self.see_calls += other.see_calls;
        self.good_captures += other.good_captures;
        self.bad_captures += other.bad_captures;
        self.pvs_zero_window_searches += other.pvs_zero_window_searches;
        self.pvs_researches += other.pvs_researches;
        self.aspiration_searches += other.aspiration_searches;
        self.see_prunes += other.see_prunes;
        self.futility_prunes += other.futility_prunes;
        self.null_move_attempts += other.null_move_attempts;
        self.null_move_fail_highs += other.null_move_fail_highs;
        self.null_move_cutoffs += other.null_move_cutoffs;
        self.null_move_verifications += other.null_move_verifications;
        self.lmr_reductions += other.lmr_reductions;
        self.lmr_researches += other.lmr_researches;
        #[cfg(feature = "stats")]
        {
            self.move_generation_calls += other.move_generation_calls;
            self.moves_generated += other.moves_generated;
            self.ordering_calls += other.ordering_calls;
            self.moves_scored += other.moves_scored;
            self.full_sorts += other.full_sorts;
            self.moves_searched += other.moves_searched;
            self.scored_moves_searched += other.scored_moves_searched;
            self.tactical_moves_searched += other.tactical_moves_searched;
            self.see_scored_moves_searched += other.see_scored_moves_searched;
            self.picker_tt_stage_visits += other.picker_tt_stage_visits;
            self.picker_good_tactical_stage_visits += other.picker_good_tactical_stage_visits;
            self.picker_killer_stage_visits += other.picker_killer_stage_visits;
            self.picker_quiet_stage_visits += other.picker_quiet_stage_visits;
            self.picker_bad_tactical_stage_visits += other.picker_bad_tactical_stage_visits;
        }
    }
}

#[derive(Default)]
pub(crate) struct SearchProgress {
    nodes: AtomicU64,
    qnodes: AtomicU64,
}

impl SearchProgress {
    fn reset(&self) {
        self.nodes.store(0, Ordering::Relaxed);
        self.qnodes.store(0, Ordering::Relaxed);
    }

    fn publish(&self, nodes: u64, qnodes: u64) {
        self.nodes.store(nodes, Ordering::Relaxed);
        self.qnodes.store(qnodes, Ordering::Relaxed);
    }

    pub(crate) fn snapshot(&self) -> (u64, u64) {
        (
            self.nodes.load(Ordering::Relaxed),
            self.qnodes.load(Ordering::Relaxed),
        )
    }
}

pub struct Searcher<'a> {
    stop: &'a AtomicBool,
    global_nodes: Option<&'a AtomicU64>,
    progress: Option<&'a SearchProgress>,
    limits: SearchLimits,
    started: Instant,
    nodes: u64,
    qnodes: u64,
    seldepth: usize,
    stopped: bool,
    hashes: Vec<u64>,
    pv: [[Move; MAX_PLY]; MAX_PLY],
    pv_len: [usize; MAX_PLY],
    tt: TranspositionTable,
    killers: [[Move; 2]; MAX_PLY],
    history: HistoryTable,
    evaluator: sanj::Evaluator,
    #[cfg(feature = "nnue")]
    accumulators: Vec<AccumulatorPair>,
    statistics: SearchStatistics,
}

impl<'a> Searcher<'a> {
    pub fn new(stop: &'a AtomicBool) -> Self {
        Self::with_hash(stop, 1)
    }

    pub fn with_hash(stop: &'a AtomicBool, hash_megabytes: usize) -> Self {
        Self::with_table(stop, TranspositionTable::new(hash_megabytes))
    }

    pub fn with_table(stop: &'a AtomicBool, tt: TranspositionTable) -> Self {
        Self::with_context(stop, tt, None, None)
    }

    pub fn with_table_and_evaluator(
        stop: &'a AtomicBool,
        tt: TranspositionTable,
        evaluator: sanj::Evaluator,
    ) -> Self {
        Self::with_evaluator_context(stop, tt, None, None, evaluator)
    }

    pub(crate) fn with_parallel_context_and_evaluator(
        stop: &'a AtomicBool,
        tt: TranspositionTable,
        global_nodes: Option<&'a AtomicU64>,
        progress: &'a SearchProgress,
        evaluator: sanj::Evaluator,
    ) -> Self {
        Self::with_evaluator_context(stop, tt, global_nodes, Some(progress), evaluator)
    }

    fn with_context(
        stop: &'a AtomicBool,
        tt: TranspositionTable,
        global_nodes: Option<&'a AtomicU64>,
        progress: Option<&'a SearchProgress>,
    ) -> Self {
        Self::with_evaluator_context(
            stop,
            tt,
            global_nodes,
            progress,
            sanj::Evaluator::classical(),
        )
    }

    fn with_evaluator_context(
        stop: &'a AtomicBool,
        tt: TranspositionTable,
        global_nodes: Option<&'a AtomicU64>,
        progress: Option<&'a SearchProgress>,
        evaluator: sanj::Evaluator,
    ) -> Self {
        Self {
            stop,
            global_nodes,
            progress,
            limits: SearchLimits::default(),
            started: Instant::now(),
            nodes: 0,
            qnodes: 0,
            seldepth: 0,
            stopped: false,
            hashes: Vec::with_capacity(MAX_PLY * 2),
            pv: [[Move::NONE; MAX_PLY]; MAX_PLY],
            pv_len: [0; MAX_PLY],
            tt,
            killers: [[Move::NONE; 2]; MAX_PLY],
            history: HistoryTable::default(),
            evaluator,
            #[cfg(feature = "nnue")]
            accumulators: Vec::with_capacity(MAX_PLY),
            statistics: SearchStatistics::default(),
        }
    }

    pub fn into_table(self) -> TranspositionTable {
        self.tt
    }

    pub fn search<F>(
        &mut self,
        position: &mut Position,
        limits: &SearchLimits,
        game_hashes: &[u64],
        on_info: F,
    ) -> SearchResult
    where
        F: FnMut(&SearchInfo),
    {
        self.search_internal(
            position,
            limits,
            game_hashes,
            SearchSetup::normal(),
            on_info,
        )
    }

    pub(crate) fn search_prepared<F>(
        &mut self,
        position: &mut Position,
        limits: &SearchLimits,
        game_hashes: &[u64],
        started: Instant,
        root_preferred: Option<Move>,
        on_info: F,
    ) -> SearchResult
    where
        F: FnMut(&SearchInfo),
    {
        self.search_internal(
            position,
            limits,
            game_hashes,
            SearchSetup::prepared(started, root_preferred),
            on_info,
        )
    }

    fn search_internal<F>(
        &mut self,
        position: &mut Position,
        limits: &SearchLimits,
        game_hashes: &[u64],
        setup: SearchSetup,
        mut on_info: F,
    ) -> SearchResult
    where
        F: FnMut(&SearchInfo),
    {
        self.reset(position, limits, game_hashes, setup);
        let root_moves = position.legal_moves();
        if root_moves.is_empty() {
            return self.root_terminal(position);
        }

        let fallback = root_moves.as_slice()[0];
        let initial_preferred = setup
            .root_preferred
            .filter(|preferred| root_moves.iter().any(|&mv| mv == *preferred))
            .unwrap_or(fallback);
        let mut best_move = initial_preferred;
        let mut best_score = self.evaluate_position(position, 0);
        let mut best_depth = 0_u8;
        let mut best_pv = vec![initial_preferred];
        let max_depth = limits.depth.unwrap_or((MAX_PLY - 2) as u8).max(1);

        for depth in 1..=max_depth {
            let mut delta = 50_i32;
            let mut alpha = if depth >= 4 {
                (best_score - delta).max(-VALUE_INFINITE)
            } else {
                -VALUE_INFINITE
            };
            let mut beta = if depth >= 4 {
                (best_score + delta).min(VALUE_INFINITE)
            } else {
                VALUE_INFINITE
            };
            let score = loop {
                self.pv_len.fill(0);
                #[cfg(feature = "stats")]
                if depth >= 4 {
                    self.statistics.aspiration_searches += 1;
                }
                let score = self.negamax(
                    position,
                    depth as i32,
                    0,
                    alpha,
                    beta,
                    SearchContext::normal(Some(best_move)),
                );
                if self.stopped || depth < 4 {
                    break score;
                }
                if score <= alpha {
                    alpha = (alpha - delta).max(-VALUE_INFINITE);
                } else if score >= beta {
                    beta = (beta + delta).min(VALUE_INFINITE);
                } else {
                    break score;
                }
                delta = (delta * 2).min(VALUE_INFINITE);
            };
            if self.stopped {
                break;
            }

            let pv = self.pv_line(0);
            if let Some(&mv) = pv.first() {
                best_move = mv;
                best_pv = pv;
            }
            best_score = score;
            best_depth = depth;
            self.publish_progress();
            let info = SearchInfo {
                depth,
                seldepth: self.seldepth.min(u8::MAX as usize) as u8,
                score,
                nodes: self.nodes,
                qnodes: self.qnodes,
                elapsed: self.started.elapsed(),
                pv: best_pv.clone(),
                hashfull: self.tt.hashfull(),
            };
            on_info(&info);

            if self.soft_limit_reached()
                || score.abs() >= VALUE_MATE - MAX_PLY as i32
                || self.node_limit_reached()
            {
                break;
            }
        }

        self.publish_progress();
        SearchResult {
            best_move: Some(best_move),
            score: best_score,
            depth: best_depth,
            seldepth: self.seldepth.min(u8::MAX as usize) as u8,
            nodes: self.nodes,
            qnodes: self.qnodes,
            elapsed: self.started.elapsed(),
            pv: best_pv,
            stopped: self.stopped,
            hashfull: self.tt.hashfull(),
            statistics: self.statistics,
        }
    }

    fn reset(
        &mut self,
        position: &mut Position,
        limits: &SearchLimits,
        game_hashes: &[u64],
        setup: SearchSetup,
    ) {
        self.limits = limits.clone();
        self.started = setup.started.unwrap_or_else(Instant::now);
        self.nodes = 0;
        self.qnodes = 0;
        self.seldepth = 0;
        self.stopped = false;
        self.statistics = SearchStatistics::default();
        self.hashes.clear();
        self.hashes.extend_from_slice(game_hashes);
        let repetition_hash = position.repetition_hash();
        if self.hashes.last().copied() != Some(repetition_hash) {
            self.hashes.push(repetition_hash);
        }
        self.pv_len.fill(0);
        if setup.advance_generation {
            self.tt.new_search();
        }
        self.killers.fill([Move::NONE; 2]);
        #[cfg(feature = "nnue")]
        {
            self.accumulators.clear();
            if let Some(network) = self.evaluator.network() {
                self.accumulators
                    .push(AccumulatorPair::refresh(position, network));
            }
        }
        if let Some(progress) = self.progress {
            progress.reset();
        }
    }

    fn root_terminal(&self, position: &Position) -> SearchResult {
        let score = if position.is_in_check(position.side_to_move()) {
            -VALUE_MATE
        } else {
            VALUE_DRAW
        };
        SearchResult {
            best_move: None,
            score,
            depth: 0,
            seldepth: 0,
            nodes: 0,
            qnodes: 0,
            elapsed: self.started.elapsed(),
            pv: Vec::new(),
            stopped: false,
            hashfull: self.tt.hashfull(),
            statistics: self.statistics,
        }
    }

    fn negamax(
        &mut self,
        position: &mut Position,
        depth: i32,
        ply: usize,
        mut alpha: i32,
        beta: i32,
        context: SearchContext,
    ) -> i32 {
        if ply >= MAX_PLY - 1 {
            return self.evaluate_position(position, ply);
        }
        self.visit_node(false);
        self.seldepth = self.seldepth.max(ply);
        self.pv_len[ply] = ply;
        if self.should_stop() {
            return VALUE_DRAW;
        }
        if !context.in_null_subtree && self.is_draw(position) {
            return VALUE_DRAW;
        }
        if depth <= 0 {
            return self.qsearch(position, ply, alpha, beta, context);
        }

        let key = position.hash();
        let original_alpha = alpha;
        let is_pv_node = beta - alpha > 1;
        let tt_data = if context.in_null_subtree {
            None
        } else {
            self.tt.probe(key, ply)
        };
        #[cfg(feature = "stats")]
        if tt_data.is_some() {
            self.statistics.tt_hits += 1;
        }
        if ply != 0
            && let Some(data) = tt_data
            && data.depth >= depth as i16
        {
            match data.bound {
                Bound::Exact => {
                    #[cfg(feature = "stats")]
                    {
                        self.statistics.tt_cutoffs += 1;
                    }
                    return data.score;
                }
                Bound::Lower if data.score >= beta => {
                    #[cfg(feature = "stats")]
                    {
                        self.statistics.tt_cutoffs += 1;
                    }
                    return data.score;
                }
                Bound::Upper if data.score <= alpha => {
                    #[cfg(feature = "stats")]
                    {
                        self.statistics.tt_cutoffs += 1;
                    }
                    return data.score;
                }
                Bound::Lower | Bound::Upper => {}
            }
        }

        let in_check = position.is_in_check(position.side_to_move());
        let moves = position.legal_moves();
        #[cfg(feature = "stats")]
        {
            self.statistics.move_generation_calls += 1;
            self.statistics.moves_generated += moves.len() as u64;
        }
        if moves.is_empty() {
            return if in_check {
                -VALUE_MATE + ply as i32
            } else {
                VALUE_DRAW
            };
        }
        let mate_bound = VALUE_MATE - MAX_PLY as i32;
        if ply != 0
            && !is_pv_node
            && context.null_allowed
            && !context.in_null_subtree
            && !in_check
            && depth >= NULL_MOVE_MIN_DEPTH
            && beta > -mate_bound
            && beta < mate_bound
            && has_meaningful_non_pawn_material(position)
            && self.evaluate_position(position, ply) >= beta
        {
            #[cfg(feature = "stats")]
            {
                self.statistics.null_move_attempts += 1;
            }
            self.push_null_accumulator(ply);
            let undo = position.make_null_move();
            let score = -self.negamax(
                position,
                depth - 1 - NULL_MOVE_REDUCTION,
                ply + 1,
                -beta,
                -beta + 1,
                context.after_null(),
            );
            position.unmake_null_move(undo);
            self.pop_accumulator(ply);
            if self.stopped {
                return VALUE_DRAW;
            }
            if score >= beta {
                #[cfg(feature = "stats")]
                {
                    self.statistics.null_move_fail_highs += 1;
                    self.statistics.null_move_cutoffs += 1;
                }
                return beta;
            }
        }
        let moving_color = position.side_to_move();
        let tt_move = tt_data.map(|data| data.best_move);
        let ordering_preferred = tt_move.or(context.preferred);
        let mut picker =
            ordering::MovePicker::main(moves, ordering_preferred, self.killers[ply], moving_color);

        let mut best = -VALUE_INFINITE;
        let mut best_move = Move::NONE;
        let mut searched_moves = MoveList::new();
        while let Some(mv) = picker.next_move(position, &self.history) {
            let move_index = searched_moves.len();
            #[cfg(feature = "stats")]
            {
                self.statistics.moves_searched += 1;
                if picker.last_move_was_scored() {
                    self.statistics.scored_moves_searched += 1;
                }
                if mv.is_capture() || mv.is_promotion() {
                    self.statistics.tactical_moves_searched += 1;
                    if picker.last_move_was_scored() {
                        self.statistics.see_scored_moves_searched += 1;
                    }
                }
            }
            #[cfg(feature = "stats")]
            if tt_move == Some(mv) {
                self.statistics.tt_move_searches += 1;
            }
            let can_reduce = ply != 0
                && depth >= 3
                && move_index >= 4
                && !is_pv_node
                && !in_check
                && !mv.is_capture()
                && !mv.is_promotion()
                && tt_move != Some(mv)
                && !self.killers[ply].contains(&mv)
                && self.history.score(moving_color, mv) < HistoryTable::MAX_SCORE / 4;
            self.push_move_accumulator(position, mv, ply);
            let undo = position.make_move(mv);
            let gives_check = can_reduce && position.is_in_check(position.side_to_move());
            if !context.in_null_subtree {
                self.hashes.push(position.repetition_hash());
            }
            let mut score;
            if move_index == 0 {
                score = -self.negamax(
                    position,
                    depth - 1,
                    ply + 1,
                    -beta,
                    -alpha,
                    context.after_move(),
                );
            } else {
                #[cfg(feature = "stats")]
                {
                    self.statistics.pvs_zero_window_searches += 1;
                }
                if can_reduce && !gives_check {
                    #[cfg(feature = "stats")]
                    {
                        self.statistics.lmr_reductions += 1;
                    }
                    score = -self.negamax(
                        position,
                        depth - 2,
                        ply + 1,
                        -alpha - 1,
                        -alpha,
                        context.after_move(),
                    );
                    if score > alpha {
                        #[cfg(feature = "stats")]
                        {
                            self.statistics.lmr_researches += 1;
                            self.statistics.pvs_zero_window_searches += 1;
                        }
                        score = -self.negamax(
                            position,
                            depth - 1,
                            ply + 1,
                            -alpha - 1,
                            -alpha,
                            context.after_move(),
                        );
                    }
                } else {
                    score = -self.negamax(
                        position,
                        depth - 1,
                        ply + 1,
                        -alpha - 1,
                        -alpha,
                        context.after_move(),
                    );
                }
                if score > alpha && score < beta {
                    #[cfg(feature = "stats")]
                    {
                        self.statistics.pvs_researches += 1;
                    }
                    score = -self.negamax(
                        position,
                        depth - 1,
                        ply + 1,
                        -beta,
                        -alpha,
                        context.after_move(),
                    );
                }
            }
            if !context.in_null_subtree {
                self.hashes.pop();
            }
            position.unmake_move(mv, undo);
            self.pop_accumulator(ply);

            if self.stopped {
                #[cfg(feature = "stats")]
                self.record_ordering_statistics(picker.statistics());
                return VALUE_DRAW;
            }
            if score > best {
                best = score;
                best_move = mv;
                self.update_pv(ply, mv);
            }
            if score > alpha {
                alpha = score;
            }
            if alpha >= beta {
                #[cfg(feature = "stats")]
                {
                    self.statistics.beta_cutoffs += 1;
                    if move_index == 0 {
                        self.statistics.first_move_beta_cutoffs += 1;
                    }
                    if mv.is_capture() || mv.is_promotion() {
                        self.statistics.capture_beta_cutoffs += 1;
                    } else {
                        self.statistics.quiet_beta_cutoffs += 1;
                    }
                }
                if !context.in_null_subtree && !mv.is_capture() && !mv.is_promotion() {
                    self.record_killer(ply, mv);
                    self.history.reward(moving_color, mv, depth);
                    for &failed in searched_moves.iter() {
                        if !failed.is_capture() && !failed.is_promotion() {
                            self.history.penalize(moving_color, failed, depth);
                        }
                    }
                }
                break;
            }
            searched_moves.push(mv);
        }
        #[cfg(feature = "stats")]
        self.record_ordering_statistics(picker.statistics());
        let bound = if best <= original_alpha {
            Bound::Upper
        } else if best >= beta {
            Bound::Lower
        } else {
            Bound::Exact
        };
        if !context.in_null_subtree {
            self.tt
                .store(key, depth as i16, best, bound, best_move, ply);
        }
        best
    }

    fn qsearch(
        &mut self,
        position: &mut Position,
        ply: usize,
        mut alpha: i32,
        beta: i32,
        context: SearchContext,
    ) -> i32 {
        if ply >= MAX_PLY - 1 {
            return self.evaluate_position(position, ply);
        }
        self.visit_node(true);
        self.seldepth = self.seldepth.max(ply);
        self.pv_len[ply] = ply;
        if self.should_stop() {
            return VALUE_DRAW;
        }
        if !context.in_null_subtree && self.is_draw(position) {
            return VALUE_DRAW;
        }

        let in_check = position.is_in_check(position.side_to_move());
        if !in_check {
            let stand_pat = self.evaluate_position(position, ply);
            if stand_pat >= beta {
                return stand_pat;
            }
            alpha = alpha.max(stand_pat);
        }

        let moves = position.legal_moves();
        #[cfg(feature = "stats")]
        {
            self.statistics.move_generation_calls += 1;
            self.statistics.moves_generated += moves.len() as u64;
        }
        if moves.is_empty() {
            return if in_check {
                -VALUE_MATE + ply as i32
            } else {
                VALUE_DRAW
            };
        }
        let moving_color = position.side_to_move();
        let mut picker =
            ordering::MovePicker::quiescence(moves, in_check, self.killers[ply], moving_color);
        #[cfg(feature = "stats")]
        let mut exhausted = true;
        while let Some(mv) = picker.next_move(position, &self.history) {
            #[cfg(feature = "stats")]
            {
                self.statistics.moves_searched += 1;
                if picker.last_move_was_scored() {
                    self.statistics.scored_moves_searched += 1;
                }
                if mv.is_capture() || mv.is_promotion() {
                    self.statistics.tactical_moves_searched += 1;
                    if picker.last_move_was_scored() {
                        self.statistics.see_scored_moves_searched += 1;
                    }
                }
            }
            self.push_move_accumulator(position, mv, ply);
            let undo = position.make_move(mv);
            if !context.in_null_subtree {
                self.hashes.push(position.repetition_hash());
            }
            let score = -self.qsearch(position, ply + 1, -beta, -alpha, context.after_move());
            if !context.in_null_subtree {
                self.hashes.pop();
            }
            position.unmake_move(mv, undo);
            self.pop_accumulator(ply);

            if self.stopped {
                #[cfg(feature = "stats")]
                self.record_ordering_statistics(picker.statistics());
                return VALUE_DRAW;
            }
            if score > alpha {
                alpha = score;
                self.update_pv(ply, mv);
                if alpha >= beta {
                    #[cfg(feature = "stats")]
                    {
                        exhausted = false;
                    }
                    break;
                }
            }
        }
        #[cfg(feature = "stats")]
        {
            if !in_check && exhausted {
                self.statistics.see_prunes += picker.bad_tactical_count() as u64;
            }
            self.record_ordering_statistics(picker.statistics());
        }
        alpha
    }

    #[inline]
    fn evaluate_position(&self, position: &Position, ply: usize) -> i32 {
        #[cfg(feature = "nnue")]
        if let Some(network) = self.evaluator.network() {
            let accumulator = self
                .accumulators
                .get(ply)
                .expect("NNUE accumulator stack must match the search ply");
            return network.evaluate_accumulator(accumulator, position.side_to_move());
        }
        let _ = ply;
        self.evaluator.evaluate(position)
    }

    #[inline]
    fn push_move_accumulator(&mut self, position: &Position, mv: Move, ply: usize) {
        #[cfg(feature = "nnue")]
        if let Some(network) = self.evaluator.network() {
            debug_assert_eq!(self.accumulators.len(), ply + 1);
            let next = self.accumulators[ply].after_move(position, mv, network);
            self.accumulators.push(next);
        }
        let _ = (position, mv, ply);
    }

    #[inline]
    fn push_null_accumulator(&mut self, ply: usize) {
        #[cfg(feature = "nnue")]
        if self.evaluator.network().is_some() {
            debug_assert_eq!(self.accumulators.len(), ply + 1);
            self.accumulators.push(self.accumulators[ply].clone());
        }
        let _ = ply;
    }

    #[inline]
    fn pop_accumulator(&mut self, ply: usize) {
        #[cfg(feature = "nnue")]
        if self.evaluator.network().is_some() {
            debug_assert_eq!(self.accumulators.len(), ply + 2);
            self.accumulators.pop();
        }
        let _ = ply;
    }

    fn update_pv(&mut self, ply: usize, mv: Move) {
        self.pv[ply][ply] = mv;
        let child_end = self.pv_len[ply + 1];
        for index in ply + 1..child_end {
            self.pv[ply][index] = self.pv[ply + 1][index];
        }
        self.pv_len[ply] = child_end.max(ply + 1);
    }

    fn record_killer(&mut self, ply: usize, mv: Move) {
        if self.killers[ply][0] != mv {
            self.killers[ply][1] = self.killers[ply][0];
            self.killers[ply][0] = mv;
        }
    }

    #[cfg(feature = "stats")]
    fn record_ordering_statistics(&mut self, statistics: ordering::OrderingStatistics) {
        self.statistics.ordering_calls += 1;
        self.statistics.moves_scored += statistics.moves_scored;
        self.statistics.full_sorts += statistics.full_sorts;
        self.statistics.see_calls += statistics.see_calls;
        self.statistics.good_captures += statistics.good_captures;
        self.statistics.bad_captures += statistics.bad_captures;
        self.statistics.picker_tt_stage_visits += statistics.tt_stage_visits;
        self.statistics.picker_good_tactical_stage_visits += statistics.good_tactical_stage_visits;
        self.statistics.picker_killer_stage_visits += statistics.killer_stage_visits;
        self.statistics.picker_quiet_stage_visits += statistics.quiet_stage_visits;
        self.statistics.picker_bad_tactical_stage_visits += statistics.bad_tactical_stage_visits;
    }

    fn pv_line(&self, ply: usize) -> Vec<Move> {
        self.pv[ply][ply..self.pv_len[ply]].to_vec()
    }

    fn is_draw(&self, position: &Position) -> bool {
        if position.halfmove_clock() >= 100 {
            return true;
        }
        let current = self
            .hashes
            .last()
            .copied()
            .expect("repetition history always contains the current position");
        self.hashes
            .iter()
            .rev()
            .take(position.halfmove_clock() as usize + 1)
            .filter(|&&hash| hash == current)
            .take(3)
            .count()
            >= 3
    }

    fn should_stop(&mut self) -> bool {
        if self.stopped {
            return true;
        }
        if self.stop.load(Ordering::Relaxed) || self.node_limit_reached() {
            self.stopped = true;
            return true;
        }
        if let Some(limit) = self.limits.hard_time {
            // A 1,024-node cadence is cheap at ordinary limits, but can
            // consume the entire budget once only a few milliseconds remain.
            // Check every node in that emergency window and always sample the
            // first node so a zero budget returns the legal root fallback.
            let time_check_due =
                self.nodes == 1 || limit <= Duration::from_millis(5) || self.nodes & 1_023 == 0;
            if time_check_due && self.started.elapsed() >= limit {
                self.stopped = true;
            }
        }
        self.stopped
    }

    fn node_limit_reached(&self) -> bool {
        self.limits.nodes.is_some_and(|limit| {
            self.global_nodes
                .map_or(self.nodes, |nodes| nodes.load(Ordering::Relaxed))
                >= limit
        })
    }

    fn soft_limit_reached(&self) -> bool {
        !self.limits.infinite
            && self
                .limits
                .soft_time
                .is_some_and(|limit| self.started.elapsed() >= limit)
    }

    fn visit_node(&mut self, quiescence: bool) {
        if let Some(nodes) = self.global_nodes {
            let limit = self
                .limits
                .nodes
                .expect("a global node counter is used only with a node limit");
            let mut current = nodes.load(Ordering::Relaxed);
            loop {
                if current >= limit {
                    self.stopped = true;
                    return;
                }
                match nodes.compare_exchange_weak(
                    current,
                    current + 1,
                    Ordering::Relaxed,
                    Ordering::Relaxed,
                ) {
                    Ok(_) => break,
                    Err(actual) => current = actual,
                }
            }
        }
        self.nodes = self.nodes.saturating_add(1);
        if quiescence {
            self.qnodes = self.qnodes.saturating_add(1);
        }
        if self.nodes & 1_023 == 0 {
            self.publish_progress();
        }
    }

    fn publish_progress(&self) {
        if let Some(progress) = self.progress {
            progress.publish(self.nodes, self.qnodes);
        }
    }
}

fn has_meaningful_non_pawn_material(position: &Position) -> bool {
    let color = position.side_to_move();
    let major = position.pieces(color, PieceType::Rook) | position.pieces(color, PieceType::Queen);
    let minor =
        position.pieces(color, PieceType::Knight) | position.pieces(color, PieceType::Bishop);
    major != 0 || minor.count_ones() >= 2
}

#[cfg(all(test, feature = "stats"))]
mod tests {
    use std::sync::atomic::AtomicBool;

    use super::{SearchContext, SearchSetup, SearchStatistics};
    use crate::{
        chess::{Move, Position},
        rekhne::{SearchLimits, Searcher, VALUE_INFINITE, VALUE_MATE, tt::Bound},
    };

    #[test]
    fn qsearch_never_prunes_losing_capture_evasions() {
        let mut position = Position::from_fen("k5r1/8/8/8/8/7q/4Q1r1/6K1 w - - 0 1")
            .expect("check-evasion fixture must be valid");
        assert!(position.is_in_check(position.side_to_move()));
        position
            .find_legal_move("e2g2")
            .expect("the losing capture must be a legal check evasion");

        let stop = AtomicBool::new(false);
        let mut searcher = Searcher::new(&stop);
        let hashes = [position.hash()];
        searcher.reset(
            &mut position,
            &SearchLimits::depth(1),
            &hashes,
            SearchSetup::normal(),
        );

        let _ = searcher.qsearch(
            &mut position,
            0,
            -VALUE_INFINITE,
            VALUE_INFINITE,
            SearchContext::normal(None),
        );

        assert!(searcher.statistics.bad_captures > 0);
        assert_eq!(searcher.statistics.see_prunes, 0);
    }

    #[test]
    fn null_move_guards_block_root_pv_shallow_check_nested_and_mate_nodes() {
        let cases = [
            (
                Position::startpos(),
                4,
                0,
                -1_000,
                -999,
                SearchContext::normal(None),
            ),
            (
                Position::startpos(),
                4,
                1,
                -1_000,
                1_000,
                SearchContext::normal(None),
            ),
            (
                Position::startpos(),
                3,
                1,
                -1_000,
                -999,
                SearchContext::normal(None),
            ),
            (
                Position::from_fen("k5r1/8/8/8/8/7q/4Q1r1/6K1 w - - 0 1")
                    .expect("in-check fixture must be valid"),
                4,
                1,
                -1_000,
                -999,
                SearchContext::normal(None),
            ),
            (
                Position::startpos(),
                4,
                1,
                -1_000,
                -999,
                SearchContext::normal(None).after_null(),
            ),
            (
                Position::startpos(),
                4,
                1,
                VALUE_MATE - super::MAX_PLY as i32 - 1,
                VALUE_MATE - super::MAX_PLY as i32,
                SearchContext::normal(None),
            ),
        ];

        for (mut position, depth, ply, alpha, beta, context) in cases {
            let original = position.clone();
            let stop = AtomicBool::new(false);
            let mut searcher = Searcher::new(&stop);
            let hashes = [position.repetition_hash()];
            searcher.reset(
                &mut position,
                &SearchLimits::depth(depth as u8),
                &hashes,
                SearchSetup::normal(),
            );

            let _ = searcher.negamax(&mut position, depth, ply, alpha, beta, context);

            assert_eq!(searcher.statistics.null_move_attempts, 0);
            assert_eq!(position, original);
        }
    }

    #[test]
    fn internal_stalemate_is_resolved_before_a_null_probe() {
        let mut position = Position::from_fen("k7/1r1N4/8/1N1Q4/8/8/8/7K b - - 0 1")
            .expect("pinned-rook stalemate fixture must be valid");
        assert!(!position.is_in_check(position.side_to_move()));
        assert!(position.legal_moves().is_empty());
        let original = position.clone();
        let stop = AtomicBool::new(false);
        let mut searcher = Searcher::new(&stop);
        let hashes = [position.repetition_hash()];
        searcher.reset(
            &mut position,
            &SearchLimits::depth(4),
            &hashes,
            SearchSetup::normal(),
        );

        let score = searcher.negamax(
            &mut position,
            4,
            1,
            -10_001,
            -10_000,
            SearchContext::normal(None),
        );

        assert_eq!(score, 0);
        assert_eq!(searcher.statistics.null_move_attempts, 0);
        assert_eq!(position, original);
    }

    #[test]
    fn synthetic_subtree_does_not_probe_store_or_train_persistent_state() {
        let mut position = Position::startpos();
        let original = position.clone();
        let legal_moves = position.legal_moves();
        let stop = AtomicBool::new(false);
        let mut searcher = Searcher::new(&stop);
        let hashes = [position.repetition_hash()];
        searcher.reset(
            &mut position,
            &SearchLimits::depth(4),
            &hashes,
            SearchSetup::normal(),
        );
        let key = position.hash();
        searcher
            .tt
            .store(key, 8, 1_234, Bound::Exact, Move::NONE, 1);
        searcher.statistics = SearchStatistics::default();

        let _ = searcher.negamax(
            &mut position,
            4,
            1,
            -1_000,
            -999,
            SearchContext::normal(None).after_null(),
        );

        assert_eq!(searcher.statistics.tt_hits, 0);
        assert_eq!(
            searcher.tt.probe(key, 1).map(|data| data.score),
            Some(1_234)
        );
        assert!(
            legal_moves
                .iter()
                .all(|&mv| searcher.history.score(position.side_to_move(), mv) == 0)
        );
        assert!(
            searcher
                .killers
                .iter()
                .flatten()
                .all(|&mv| mv == Move::NONE)
        );
        assert_eq!(position, original);
    }
}

#[cfg(all(test, feature = "stats"))]
#[path = "zugzwang_tests.rs"]
mod zugzwang_tests;
