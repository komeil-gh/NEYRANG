use crate::chess::{Color, Move, MoveList, PieceType, Position};

use super::{
    continuation_history::{ContinuationContext, ContinuationHistory},
    history::HistoryTable,
    see::see,
};

const PIECE_VALUE: [i32; 6] = [100, 320, 330, 500, 900, 20_000];
const GOOD_TACTICAL_SCORE: i32 = 200_000;
const BAD_CAPTURE_SCORE: i32 = -100_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum MoveClass {
    Unclassified,
    GoodTactical,
    Killer,
    Quiet,
    BadTactical,
    Taken,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Stage {
    Preferred,
    GoodTactical,
    Killer,
    Quiet,
    BadTactical,
    Done,
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct OrderingStatistics {
    pub bad_capture_count: usize,
    #[cfg(feature = "stats")]
    pub moves_scored: u64,
    #[cfg(feature = "stats")]
    pub full_sorts: u64,
    #[cfg(feature = "stats")]
    pub see_calls: u64,
    #[cfg(feature = "stats")]
    pub good_captures: u64,
    #[cfg(feature = "stats")]
    pub bad_captures: u64,
    #[cfg(feature = "stats")]
    pub tt_stage_visits: u64,
    #[cfg(feature = "stats")]
    pub good_tactical_stage_visits: u64,
    #[cfg(feature = "stats")]
    pub killer_stage_visits: u64,
    #[cfg(feature = "stats")]
    pub quiet_stage_visits: u64,
    #[cfg(feature = "stats")]
    pub bad_tactical_stage_visits: u64,
    #[cfg(feature = "stats")]
    pub continuation_probes: u64,
    #[cfg(feature = "stats")]
    pub continuation_nonzero_probes: u64,
    #[cfg(feature = "stats")]
    pub continuation_reorderings: u64,
}

/// Fixed-storage staged move delivery. Tactical moves retain NEYRANG's exact SEE
/// classification; later stages are not scored until search asks for them.
pub(crate) struct MovePicker {
    moves: MoveList,
    classes: [MoveClass; MoveList::CAPACITY],
    scores: [i32; MoveList::CAPACITY],
    preferred: Option<Move>,
    killers: [Move; 2],
    color: Color,
    previous_context: Option<ContinuationContext>,
    tactical_only: bool,
    stage: Stage,
    stage_initialized: bool,
    last_move_was_scored: bool,
    statistics: OrderingStatistics,
}

impl MovePicker {
    pub(crate) fn main(
        moves: MoveList,
        preferred: Option<Move>,
        killers: [Move; 2],
        color: Color,
        previous_context: Option<ContinuationContext>,
    ) -> Self {
        Self::new(moves, preferred, killers, color, previous_context, false)
    }

    pub(crate) fn quiescence(
        moves: MoveList,
        in_check: bool,
        killers: [Move; 2],
        color: Color,
    ) -> Self {
        Self::new(moves, None, killers, color, None, !in_check)
    }

    fn new(
        moves: MoveList,
        preferred: Option<Move>,
        killers: [Move; 2],
        color: Color,
        previous_context: Option<ContinuationContext>,
        tactical_only: bool,
    ) -> Self {
        Self {
            moves,
            classes: [MoveClass::Unclassified; MoveList::CAPACITY],
            scores: [i32::MIN; MoveList::CAPACITY],
            preferred,
            killers,
            color,
            previous_context,
            tactical_only,
            stage: Stage::Preferred,
            stage_initialized: false,
            last_move_was_scored: false,
            statistics: OrderingStatistics::default(),
        }
    }

    pub(crate) fn next_move(
        &mut self,
        position: &Position,
        history: &HistoryTable,
        continuation_history: &ContinuationHistory,
    ) -> Option<Move> {
        self.last_move_was_scored = false;
        loop {
            match self.stage {
                Stage::Preferred => {
                    if !self.stage_initialized {
                        #[cfg(feature = "stats")]
                        {
                            self.statistics.tt_stage_visits += 1;
                        }
                        self.stage_initialized = true;
                    }
                    if let Some(preferred) = self.preferred.take()
                        && let Some(index) = self.find_unclassified(preferred)
                    {
                        self.classes[index] = MoveClass::Taken;
                        self.advance(Stage::GoodTactical);
                        return Some(preferred);
                    }
                    self.advance(Stage::GoodTactical);
                }
                Stage::GoodTactical => {
                    if !self.stage_initialized {
                        #[cfg(feature = "stats")]
                        {
                            self.statistics.good_tactical_stage_visits += 1;
                        }
                        self.classify_tacticals(position);
                        self.stage_initialized = true;
                    }
                    if let Some(mv) = self.pick_best(MoveClass::GoodTactical) {
                        self.last_move_was_scored = true;
                        return Some(mv);
                    }
                    self.advance(if self.tactical_only {
                        Stage::Done
                    } else {
                        Stage::Killer
                    });
                }
                Stage::Killer => {
                    if !self.stage_initialized {
                        #[cfg(feature = "stats")]
                        {
                            self.statistics.killer_stage_visits += 1;
                        }
                        self.classify_killers(position, history, continuation_history);
                        self.stage_initialized = true;
                    }
                    if let Some(mv) = self.pick_best(MoveClass::Killer) {
                        self.last_move_was_scored = true;
                        return Some(mv);
                    }
                    self.advance(Stage::Quiet);
                }
                Stage::Quiet => {
                    if !self.stage_initialized {
                        #[cfg(feature = "stats")]
                        {
                            self.statistics.quiet_stage_visits += 1;
                        }
                        self.classify_quiets(position, history, continuation_history);
                        self.stage_initialized = true;
                    }
                    if let Some(mv) = self.pick_best(MoveClass::Quiet) {
                        self.last_move_was_scored = true;
                        return Some(mv);
                    }
                    self.advance(Stage::BadTactical);
                }
                Stage::BadTactical => {
                    if !self.stage_initialized {
                        #[cfg(feature = "stats")]
                        {
                            self.statistics.bad_tactical_stage_visits += 1;
                        }
                        self.stage_initialized = true;
                    }
                    if let Some(mv) = self.pick_best(MoveClass::BadTactical) {
                        self.last_move_was_scored = true;
                        return Some(mv);
                    }
                    self.advance(Stage::Done);
                }
                Stage::Done => return None,
            }
        }
    }

    #[inline]
    #[cfg(any(feature = "stats", test))]
    pub(crate) const fn last_move_was_scored(&self) -> bool {
        self.last_move_was_scored
    }

    #[inline]
    #[cfg(any(feature = "stats", test))]
    pub(crate) const fn bad_tactical_count(&self) -> usize {
        self.statistics.bad_capture_count
    }

    #[inline]
    #[cfg(any(feature = "stats", test))]
    pub(crate) const fn statistics(&self) -> OrderingStatistics {
        self.statistics
    }

    fn advance(&mut self, stage: Stage) {
        self.stage = stage;
        self.stage_initialized = false;
    }

    fn find_unclassified(&self, expected: Move) -> Option<usize> {
        self.moves
            .iter()
            .enumerate()
            .find(|&(index, &mv)| self.classes[index] == MoveClass::Unclassified && mv == expected)
            .map(|(index, _)| index)
    }

    fn classify_tacticals(&mut self, position: &Position) {
        for (index, &mv) in self.moves.iter().enumerate() {
            if self.classes[index] != MoveClass::Unclassified
                || (!mv.is_capture() && !mv.is_promotion())
            {
                continue;
            }
            let (score, is_good) = tactical_score(position, mv, &mut self.statistics);
            self.scores[index] = score;
            self.classes[index] = if is_good {
                MoveClass::GoodTactical
            } else {
                self.statistics.bad_capture_count += 1;
                MoveClass::BadTactical
            };
        }
    }

    fn classify_killers(
        &mut self,
        position: &Position,
        history: &HistoryTable,
        continuation_history: &ContinuationHistory,
    ) {
        #[cfg(feature = "stats")]
        let mut parent_scores = [i32::MIN; MoveList::CAPACITY];
        let previous_context = self.previous_context;
        for (index, &mv) in self.moves.iter().enumerate() {
            if self.classes[index] != MoveClass::Unclassified || !self.killers.contains(&mv) {
                continue;
            }
            #[cfg(feature = "stats")]
            let parent_score = quiet_score(mv, self.killers, history, self.color, 0);
            let continuation_score = continuation_score(
                previous_context,
                position,
                mv,
                continuation_history,
                &mut self.statistics,
            );
            self.scores[index] =
                quiet_score(mv, self.killers, history, self.color, continuation_score);
            self.classes[index] = MoveClass::Killer;
            #[cfg(feature = "stats")]
            {
                parent_scores[index] = parent_score;
                self.statistics.moves_scored += 1;
            }
        }
        #[cfg(feature = "stats")]
        {
            self.statistics.continuation_reorderings +=
                self.pairwise_reorderings(MoveClass::Killer, &parent_scores);
        }
    }

    fn classify_quiets(
        &mut self,
        position: &Position,
        history: &HistoryTable,
        continuation_history: &ContinuationHistory,
    ) {
        #[cfg(feature = "stats")]
        let mut parent_scores = [i32::MIN; MoveList::CAPACITY];
        let previous_context = self.previous_context;
        for (index, &mv) in self.moves.iter().enumerate() {
            if self.classes[index] != MoveClass::Unclassified {
                continue;
            }
            #[cfg(feature = "stats")]
            let parent_score = quiet_score(mv, self.killers, history, self.color, 0);
            let continuation_score = continuation_score(
                previous_context,
                position,
                mv,
                continuation_history,
                &mut self.statistics,
            );
            self.scores[index] =
                quiet_score(mv, self.killers, history, self.color, continuation_score);
            self.classes[index] = MoveClass::Quiet;
            #[cfg(feature = "stats")]
            {
                parent_scores[index] = parent_score;
                self.statistics.moves_scored += 1;
            }
        }
        #[cfg(feature = "stats")]
        {
            self.statistics.continuation_reorderings +=
                self.pairwise_reorderings(MoveClass::Quiet, &parent_scores);
        }
    }

    #[cfg(feature = "stats")]
    fn pairwise_reorderings(
        &self,
        class: MoveClass,
        parent_scores: &[i32; MoveList::CAPACITY],
    ) -> u64 {
        let mut reorderings = 0_u64;
        for first in 0..self.moves.len() {
            if self.classes[first] != class {
                continue;
            }
            for second in first + 1..self.moves.len() {
                if self.classes[second] != class {
                    continue;
                }
                let parent_prefers_first = parent_scores[first] >= parent_scores[second];
                let candidate_prefers_first = self.scores[first] >= self.scores[second];
                if parent_prefers_first != candidate_prefers_first {
                    reorderings += 1;
                }
            }
        }
        reorderings
    }

    fn pick_best(&mut self, class: MoveClass) -> Option<Move> {
        let mut best_index = None;
        let mut best_score = i32::MIN;
        for index in 0..self.moves.len() {
            if self.classes[index] == class
                && (best_index.is_none() || self.scores[index] > best_score)
            {
                best_index = Some(index);
                best_score = self.scores[index];
            }
        }
        let index = best_index?;
        self.classes[index] = MoveClass::Taken;
        Some(self.moves.as_slice()[index])
    }
}

fn tactical_score(
    position: &Position,
    mv: Move,
    _statistics: &mut OrderingStatistics,
) -> (i32, bool) {
    #[cfg(feature = "stats")]
    {
        _statistics.moves_scored += 1;
        _statistics.see_calls += 1;
    }
    let exchange = see(position, mv);
    let promotion_bonus = mv
        .promotion()
        .map_or(0, |promotion| PIECE_VALUE[promotion.index()] * 16);
    let victim = if mv.is_en_passant() {
        PieceType::Pawn
    } else {
        position
            .piece_at(mv.to())
            .map_or(PieceType::Pawn, |(_, kind)| kind)
    };
    let attacker = position
        .piece_at(mv.from())
        .map_or(PieceType::Pawn, |(_, kind)| kind);
    let mvv_lva = if mv.is_capture() {
        PIECE_VALUE[victim.index()] * 16 - PIECE_VALUE[attacker.index()]
    } else {
        0
    };
    if mv.is_promotion() || exchange >= 0 {
        #[cfg(feature = "stats")]
        if mv.is_capture() {
            _statistics.good_captures += 1;
        }
        (
            GOOD_TACTICAL_SCORE + promotion_bonus + mvv_lva + exchange,
            true,
        )
    } else {
        #[cfg(feature = "stats")]
        {
            _statistics.bad_captures += 1;
        }
        (BAD_CAPTURE_SCORE + mvv_lva + exchange, false)
    }
}

fn quiet_score(
    mv: Move,
    killers: [Move; 2],
    history: &HistoryTable,
    color: Color,
    continuation_score: i32,
) -> i32 {
    let mut score = 0;
    if mv.is_castle() {
        score += 500;
    }
    if killers[0] == mv {
        score += 90_000;
    } else if killers[1] == mv {
        score += 80_000;
    }
    score + history.score(color, mv) + continuation_score / 2
}

fn continuation_score(
    previous: Option<ContinuationContext>,
    position: &Position,
    mv: Move,
    continuation_history: &ContinuationHistory,
    _statistics: &mut OrderingStatistics,
) -> i32 {
    let Some(previous) = previous else {
        return 0;
    };
    let current = ContinuationContext::from_move(position, mv);
    let score = continuation_history.score(Some(previous), current);
    #[cfg(feature = "stats")]
    {
        _statistics.continuation_probes += 1;
        if score != 0 {
            _statistics.continuation_nonzero_probes += 1;
        }
    }
    score
}

#[cfg(test)]
mod tests {
    use crate::{
        chess::{Color, Move, PieceType, Position, Square},
        search::{
            continuation_history::{ContinuationContext, ContinuationHistory},
            see,
        },
    };

    use super::{HistoryTable, MovePicker, OrderingStatistics, quiet_score, tactical_score};

    fn collect(
        picker: &mut MovePicker,
        position: &Position,
        history: &HistoryTable,
        continuation_history: &ContinuationHistory,
    ) -> Vec<Move> {
        let mut picked = Vec::new();
        while let Some(mv) = picker.next_move(position, history, continuation_history) {
            picked.push(mv);
        }
        picked
    }

    fn index_of(moves: &[Move], expected: Move) -> usize {
        moves
            .iter()
            .position(|&mv| mv == expected)
            .expect("expected move must remain in the picked list")
    }

    #[test]
    fn stages_bad_captures_after_killers_and_quiets() {
        let mut position = Position::from_fen("6k1/8/5p2/3qp3/2P1Q3/8/8/6K1 w - - 0 1")
            .expect("ordering fixture must be valid");
        let preferred = position
            .find_legal_move("e4d3")
            .expect("preferred quiet move must be legal");
        let good_capture = position
            .find_legal_move("c4d5")
            .expect("winning capture must be legal");
        let bad_capture = position
            .find_legal_move("e4e5")
            .expect("losing capture must be legal");
        let killer = position
            .find_legal_move("g1f2")
            .expect("killer move must be legal");
        let quiet = position
            .find_legal_move("c4c5")
            .expect("ordinary quiet move must be legal");
        let legal = position.legal_moves();
        let history = HistoryTable::default();
        let previous = ContinuationContext::new(Color::Black, PieceType::Knight, Square::F6);
        let mut continuation_history = ContinuationHistory::default();
        let killer_context = ContinuationContext::from_move(&position, killer);
        let quiet_context = ContinuationContext::from_move(&position, quiet);
        for _ in 0..10_000 {
            continuation_history.penalize(previous, killer_context, 16);
            continuation_history.reward(previous, quiet_context, 16);
        }
        let mut picker = MovePicker::main(
            legal.clone(),
            Some(preferred),
            [killer, Move::NONE],
            position.side_to_move(),
            Some(previous),
        );

        let picked = collect(&mut picker, &position, &history, &continuation_history);

        assert_eq!(picked.len(), legal.len());
        assert_eq!(picked[0], preferred);
        assert!(index_of(&picked, good_capture) < index_of(&picked, killer));
        assert!(index_of(&picked, killer) < index_of(&picked, quiet));
        assert!(index_of(&picked, quiet) < index_of(&picked, bad_capture));
        assert_eq!(picker.bad_tactical_count(), 1);
        for (index, &mv) in picked.iter().enumerate() {
            assert!(!picked[..index].contains(&mv), "move returned twice: {mv}");
        }
        #[cfg(feature = "stats")]
        {
            assert_eq!(picker.statistics().see_calls, 3);
            assert_eq!(picker.statistics().good_captures, 2);
            assert_eq!(picker.statistics().bad_captures, 1);
            assert_eq!(picker.statistics().full_sorts, 0);
        }
    }

    #[test]
    fn preferred_move_defers_all_later_scoring_and_is_never_duplicated() {
        let mut position = Position::from_fen("6k1/8/5p2/3qp3/2P1Q3/8/8/6K1 w - - 0 1")
            .expect("ordering fixture must be valid");
        let preferred = position
            .find_legal_move("e4d3")
            .expect("preferred move must be legal");
        let history = HistoryTable::default();
        let continuation_history = ContinuationHistory::default();
        let mut picker = MovePicker::main(
            position.legal_moves(),
            Some(preferred),
            [preferred, preferred],
            position.side_to_move(),
            None,
        );

        assert_eq!(
            picker.next_move(&position, &history, &continuation_history),
            Some(preferred)
        );
        assert!(!picker.last_move_was_scored());
        #[cfg(feature = "stats")]
        {
            let statistics = picker.statistics();
            assert_eq!(statistics.moves_scored, 0);
            assert_eq!(statistics.see_calls, 0);
            assert_eq!(statistics.tt_stage_visits, 1);
            assert_eq!(statistics.good_tactical_stage_visits, 0);
        }

        let rest = collect(&mut picker, &position, &history, &continuation_history);
        assert!(!rest.contains(&preferred));
        #[cfg(feature = "stats")]
        assert!(picker.statistics().see_calls > 0);
    }

    #[test]
    fn continuation_reorders_moves_only_inside_the_quiet_stage() {
        let mut position = Position::startpos();
        let legal = position.legal_moves();
        let quiets = legal
            .iter()
            .copied()
            .filter(|mv| !mv.is_capture() && !mv.is_promotion())
            .collect::<Vec<_>>();
        let parent_first = quiets[0];
        let boosted_reply = quiets[1];
        let previous = ContinuationContext::new(Color::Black, PieceType::Knight, Square::F6);
        let parent_first_context = ContinuationContext::from_move(&position, parent_first);
        let boosted_context = ContinuationContext::from_move(&position, boosted_reply);
        let mut continuation_history = ContinuationHistory::default();
        for _ in 0..10_000 {
            continuation_history.penalize(previous, parent_first_context, 16);
            continuation_history.reward(previous, boosted_context, 16);
        }
        let history = HistoryTable::default();
        let mut picker = MovePicker::main(
            legal,
            None,
            [Move::NONE; 2],
            position.side_to_move(),
            Some(previous),
        );

        let picked = collect(&mut picker, &position, &history, &continuation_history);

        assert!(index_of(&picked, boosted_reply) < index_of(&picked, parent_first));
        #[cfg(feature = "stats")]
        {
            let statistics = picker.statistics();
            assert!(statistics.continuation_probes > 0);
            assert!(statistics.continuation_nonzero_probes >= 2);
            assert!(statistics.continuation_reorderings > 0);
        }
    }

    #[test]
    fn qsearch_outside_check_returns_only_existing_good_tacticals() {
        let mut position = Position::from_fen("6k1/8/5p2/3qp3/2P1Q3/8/8/6K1 w - - 0 1")
            .expect("ordering fixture must be valid");
        let bad_capture = position
            .find_legal_move("e4e5")
            .expect("losing capture must be legal");
        let history = HistoryTable::default();
        let continuation_history = ContinuationHistory::default();
        let mut picker = MovePicker::quiescence(
            position.legal_moves(),
            false,
            [Move::NONE; 2],
            position.side_to_move(),
        );

        let picked = collect(&mut picker, &position, &history, &continuation_history);

        assert!(!picked.contains(&bad_capture));
        assert_eq!(picker.bad_tactical_count(), 1);
        assert!(picked.iter().all(|&mv| {
            (mv.is_capture() || mv.is_promotion()) && (mv.is_promotion() || see(&position, mv) >= 0)
        }));
        #[cfg(feature = "stats")]
        assert_eq!(picker.statistics().continuation_probes, 0);
    }

    #[test]
    fn qsearch_in_check_returns_every_legal_evasion_once() {
        let mut position = Position::from_fen("k5r1/8/8/8/8/7q/4Q1r1/6K1 w - - 0 1")
            .expect("check-evasion fixture must be valid");
        assert!(position.is_in_check(position.side_to_move()));
        let legal = position.legal_moves();
        let history = HistoryTable::default();
        let continuation_history = ContinuationHistory::default();
        let mut picker = MovePicker::quiescence(
            legal.clone(),
            true,
            [Move::NONE; 2],
            position.side_to_move(),
        );

        let picked = collect(&mut picker, &position, &history, &continuation_history);

        assert_eq!(picked.len(), legal.len());
        for &mv in legal.iter() {
            assert_eq!(
                picked.iter().filter(|&&candidate| candidate == mv).count(),
                1
            );
        }
        #[cfg(feature = "stats")]
        assert_eq!(picker.statistics().continuation_probes, 0);
    }

    #[test]
    fn main_picker_preserves_every_legal_move_across_deterministic_play() {
        let mut position = Position::startpos();
        let history = HistoryTable::default();
        let continuation_history = ContinuationHistory::default();
        let mut state = 0xA3C5_9AC3_2026_0829_u64;

        for sample in 0..96 {
            let legal = position.legal_moves();
            if legal.is_empty() {
                position = Position::startpos();
                continue;
            }
            let preferred_index = (state as usize) % legal.len();
            let preferred = legal.as_slice()[preferred_index];
            let quiets = legal
                .iter()
                .copied()
                .filter(|mv| !mv.is_capture() && !mv.is_promotion())
                .collect::<Vec<_>>();
            let killers = [
                quiets.first().copied().unwrap_or(Move::NONE),
                quiets.last().copied().unwrap_or(Move::NONE),
            ];
            let mut picker = MovePicker::main(
                legal.clone(),
                Some(preferred),
                killers,
                position.side_to_move(),
                None,
            );

            let picked = collect(&mut picker, &position, &history, &continuation_history);

            assert_eq!(picked.len(), legal.len(), "sample {sample}");
            assert_eq!(picked[0], preferred, "sample {sample}");
            let mut score_statistics = OrderingStatistics::default();
            let ordered_scores = picked
                .iter()
                .map(|&mv| {
                    if mv == preferred {
                        1_000_000
                    } else if mv.is_capture() || mv.is_promotion() {
                        tactical_score(&position, mv, &mut score_statistics).0
                    } else {
                        quiet_score(mv, killers, &history, position.side_to_move(), 0)
                    }
                })
                .collect::<Vec<_>>();
            assert!(
                ordered_scores
                    .windows(2)
                    .all(|scores| scores[0] >= scores[1]),
                "legacy score inversion at sample {sample}: {ordered_scores:?}"
            );
            for &mv in legal.iter() {
                assert_eq!(
                    picked.iter().filter(|&&candidate| candidate == mv).count(),
                    1,
                    "sample {sample}, move {mv}"
                );
            }

            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            let advance = legal.as_slice()[(state as usize) % legal.len()];
            position.make_move(advance);
        }
    }
}
