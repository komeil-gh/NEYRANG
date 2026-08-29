//! Iterative search driver and recursive alpha-beta implementation.

use std::{
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};

use crate::{
    chess::{Move, MoveList, Position},
    eval,
};

use super::{
    SearchLimits,
    history::HistoryTable,
    ordering,
    tt::{Bound, TranspositionTable},
};

pub const MAX_PLY: usize = 128;
pub const VALUE_DRAW: i32 = 0;
pub const VALUE_MATE: i32 = 30_000;
pub const VALUE_INFINITE: i32 = 32_000;

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
    #[cfg(feature = "stats")]
    pub see_exchange_steps: u64,
    #[cfg(feature = "stats")]
    pub see_ge_calls: u64,
    #[cfg(feature = "stats")]
    pub see_ge_early_exits: u64,
    #[cfg(feature = "stats")]
    pub see_ge_exchange_steps: u64,
    pub good_captures: u64,
    pub bad_captures: u64,
    pub pvs_zero_window_searches: u64,
    pub pvs_researches: u64,
    pub aspiration_searches: u64,
    pub see_prunes: u64,
    pub futility_prunes: u64,
    pub null_move_cutoffs: u64,
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
    pub tactical_candidates_untested: u64,
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

#[cfg(feature = "stats")]
impl SearchStatistics {
    pub fn moves_scored_unused(self) -> u64 {
        self.moves_scored.saturating_sub(self.scored_moves_searched)
    }

    pub fn see_scored_moves_unused(self) -> u64 {
        (self.see_calls + self.see_ge_calls).saturating_sub(self.see_scored_moves_searched)
    }

    pub fn total_see_exchange_steps(self) -> u64 {
        self.see_exchange_steps + self.see_ge_exchange_steps
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
        self.see_exchange_steps += other.see_exchange_steps;
        self.see_ge_calls += other.see_ge_calls;
        self.see_ge_early_exits += other.see_ge_early_exits;
        self.see_ge_exchange_steps += other.see_ge_exchange_steps;
        self.good_captures += other.good_captures;
        self.bad_captures += other.bad_captures;
        self.pvs_zero_window_searches += other.pvs_zero_window_searches;
        self.pvs_researches += other.pvs_researches;
        self.aspiration_searches += other.aspiration_searches;
        self.see_prunes += other.see_prunes;
        self.futility_prunes += other.futility_prunes;
        self.null_move_cutoffs += other.null_move_cutoffs;
        self.lmr_reductions += other.lmr_reductions;
        self.lmr_researches += other.lmr_researches;
        self.move_generation_calls += other.move_generation_calls;
        self.moves_generated += other.moves_generated;
        self.ordering_calls += other.ordering_calls;
        self.moves_scored += other.moves_scored;
        self.full_sorts += other.full_sorts;
        self.moves_searched += other.moves_searched;
        self.scored_moves_searched += other.scored_moves_searched;
        self.tactical_moves_searched += other.tactical_moves_searched;
        self.see_scored_moves_searched += other.see_scored_moves_searched;
        self.tactical_candidates_untested += other.tactical_candidates_untested;
        self.picker_tt_stage_visits += other.picker_tt_stage_visits;
        self.picker_good_tactical_stage_visits += other.picker_good_tactical_stage_visits;
        self.picker_killer_stage_visits += other.picker_killer_stage_visits;
        self.picker_quiet_stage_visits += other.picker_quiet_stage_visits;
        self.picker_bad_tactical_stage_visits += other.picker_bad_tactical_stage_visits;
    }
}

pub struct Searcher<'a> {
    stop: &'a AtomicBool,
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
        Self {
            stop,
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
        mut on_info: F,
    ) -> SearchResult
    where
        F: FnMut(&SearchInfo),
    {
        self.reset(position, limits, game_hashes);
        let root_moves = position.legal_moves();
        if root_moves.is_empty() {
            return self.root_terminal(position);
        }

        let fallback = root_moves.as_slice()[0];
        let mut best_move = fallback;
        let mut best_score = eval::evaluate(position);
        let mut best_depth = 0_u8;
        let mut best_pv = vec![fallback];
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
                let score = self.negamax(position, depth as i32, 0, alpha, beta, Some(best_move));
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

    fn reset(&mut self, position: &mut Position, limits: &SearchLimits, game_hashes: &[u64]) {
        self.limits = limits.clone();
        self.started = Instant::now();
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
        self.tt.new_search();
        self.killers.fill([Move::NONE; 2]);
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
        preferred: Option<Move>,
    ) -> i32 {
        if ply >= MAX_PLY - 1 {
            return eval::evaluate(position);
        }
        self.nodes = self.nodes.saturating_add(1);
        self.seldepth = self.seldepth.max(ply);
        self.pv_len[ply] = ply;
        if self.should_stop() {
            return VALUE_DRAW;
        }
        if self.is_draw(position) {
            return VALUE_DRAW;
        }
        if depth <= 0 {
            return self.qsearch(position, ply, alpha, beta);
        }

        let key = position.hash();
        let original_alpha = alpha;
        let is_pv_node = beta - alpha > 1;
        let tt_data = self.tt.probe(key, ply);
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
        let moving_color = position.side_to_move();
        let tt_move = tt_data.map(|data| data.best_move);
        let ordering_preferred = tt_move.or(preferred);
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
                    if picker.last_move_was_see_tested() {
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
            let undo = position.make_move(mv);
            let gives_check = can_reduce && position.is_in_check(position.side_to_move());
            self.hashes.push(position.repetition_hash());
            let mut score;
            if move_index == 0 {
                score = -self.negamax(position, depth - 1, ply + 1, -beta, -alpha, None);
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
                    score = -self.negamax(position, depth - 2, ply + 1, -alpha - 1, -alpha, None);
                    if score > alpha {
                        #[cfg(feature = "stats")]
                        {
                            self.statistics.lmr_researches += 1;
                            self.statistics.pvs_zero_window_searches += 1;
                        }
                        score =
                            -self.negamax(position, depth - 1, ply + 1, -alpha - 1, -alpha, None);
                    }
                } else {
                    score = -self.negamax(position, depth - 1, ply + 1, -alpha - 1, -alpha, None);
                }
                if score > alpha && score < beta {
                    #[cfg(feature = "stats")]
                    {
                        self.statistics.pvs_researches += 1;
                    }
                    score = -self.negamax(position, depth - 1, ply + 1, -beta, -alpha, None);
                }
            }
            self.hashes.pop();
            position.unmake_move(mv, undo);

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
                if !mv.is_capture() && !mv.is_promotion() {
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
        self.tt
            .store(key, depth as i16, best, bound, best_move, ply);
        best
    }

    fn qsearch(&mut self, position: &mut Position, ply: usize, mut alpha: i32, beta: i32) -> i32 {
        if ply >= MAX_PLY - 1 {
            return eval::evaluate(position);
        }
        self.nodes = self.nodes.saturating_add(1);
        self.qnodes = self.qnodes.saturating_add(1);
        self.seldepth = self.seldepth.max(ply);
        self.pv_len[ply] = ply;
        if self.should_stop() {
            return VALUE_DRAW;
        }
        if self.is_draw(position) {
            return VALUE_DRAW;
        }

        let in_check = position.is_in_check(position.side_to_move());
        if !in_check {
            let stand_pat = eval::evaluate(position);
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
                    if picker.last_move_was_see_tested() {
                        self.statistics.see_scored_moves_searched += 1;
                    }
                }
            }
            let undo = position.make_move(mv);
            self.hashes.push(position.repetition_hash());
            let score = -self.qsearch(position, ply + 1, -beta, -alpha);
            self.hashes.pop();
            position.unmake_move(mv, undo);

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
        self.statistics.see_exchange_steps += statistics.see_exchange_steps;
        self.statistics.see_ge_calls += statistics.see_ge_calls;
        self.statistics.see_ge_early_exits += statistics.see_ge_early_exits;
        self.statistics.see_ge_exchange_steps += statistics.see_ge_exchange_steps;
        self.statistics.tactical_candidates_untested += statistics.tactical_candidates_untested;
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
        self.limits.nodes.is_some_and(|limit| self.nodes >= limit)
    }

    fn soft_limit_reached(&self) -> bool {
        !self.limits.infinite
            && self
                .limits
                .soft_time
                .is_some_and(|limit| self.started.elapsed() >= limit)
    }
}

#[cfg(all(test, feature = "stats"))]
mod tests {
    use std::sync::atomic::AtomicBool;

    use crate::{
        chess::Position,
        search::{SearchLimits, Searcher, VALUE_INFINITE},
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
        searcher.reset(&mut position, &SearchLimits::depth(1), &hashes);

        let _ = searcher.qsearch(&mut position, 0, -VALUE_INFINITE, VALUE_INFINITE);

        assert!(searcher.statistics.bad_captures > 0);
        assert_eq!(searcher.statistics.see_prunes, 0);
    }
}
