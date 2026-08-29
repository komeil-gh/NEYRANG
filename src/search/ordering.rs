use crate::chess::{Color, Move, MoveList, PieceType, Position};

use super::history::HistoryTable;
#[cfg(not(feature = "stats"))]
use super::see::{see, see_ge};
#[cfg(feature = "stats")]
use super::see::{see_ge_with_work, see_with_work};

const PIECE_VALUE: [i32; 6] = [100, 320, 330, 500, 900, 20_000];
const GOOD_TACTICAL_SCORE: i32 = 200_000;
const BAD_CAPTURE_SCORE: i32 = -100_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum MoveClass {
    Unclassified,
    TacticalCandidate,
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
    pub see_exchange_steps: u64,
    #[cfg(feature = "stats")]
    pub see_ge_calls: u64,
    #[cfg(feature = "stats")]
    pub see_ge_early_exits: u64,
    #[cfg(feature = "stats")]
    pub see_ge_exchange_steps: u64,
    #[cfg(feature = "stats")]
    pub tactical_candidates_untested: u64,
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
    tactical_only: bool,
    lazy_tacticals: bool,
    stage: Stage,
    stage_initialized: bool,
    last_move_was_scored: bool,
    #[cfg(any(feature = "stats", test))]
    last_move_was_see_tested: bool,
    statistics: OrderingStatistics,
}

impl MovePicker {
    pub(crate) fn main(
        moves: MoveList,
        preferred: Option<Move>,
        killers: [Move; 2],
        color: Color,
    ) -> Self {
        Self::new(moves, preferred, killers, color, false, true)
    }

    pub(crate) fn quiescence(
        moves: MoveList,
        in_check: bool,
        killers: [Move; 2],
        color: Color,
    ) -> Self {
        Self::new(moves, None, killers, color, !in_check, false)
    }

    fn new(
        moves: MoveList,
        preferred: Option<Move>,
        killers: [Move; 2],
        color: Color,
        tactical_only: bool,
        lazy_tacticals: bool,
    ) -> Self {
        Self {
            moves,
            classes: [MoveClass::Unclassified; MoveList::CAPACITY],
            scores: [i32::MIN; MoveList::CAPACITY],
            preferred,
            killers,
            color,
            tactical_only,
            lazy_tacticals,
            stage: Stage::Preferred,
            stage_initialized: false,
            last_move_was_scored: false,
            #[cfg(any(feature = "stats", test))]
            last_move_was_see_tested: false,
            statistics: OrderingStatistics::default(),
        }
    }

    pub(crate) fn next_move(
        &mut self,
        position: &Position,
        history: &HistoryTable,
    ) -> Option<Move> {
        self.last_move_was_scored = false;
        #[cfg(any(feature = "stats", test))]
        {
            self.last_move_was_see_tested = false;
        }
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
                        if self.lazy_tacticals {
                            self.classify_tactical_candidates(position);
                        } else {
                            self.classify_tacticals_exact(position);
                        }
                        self.stage_initialized = true;
                    }
                    let picked = if self.lazy_tacticals {
                        self.pick_next_good_tactical(position)
                    } else {
                        self.pick_best(MoveClass::GoodTactical)
                    };
                    if let Some(mv) = picked {
                        self.last_move_was_scored = true;
                        #[cfg(any(feature = "stats", test))]
                        {
                            self.last_move_was_see_tested =
                                !self.lazy_tacticals || !mv.is_promotion();
                        }
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
                        self.classify_killers(history);
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
                        self.classify_quiets(history);
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
                        #[cfg(any(feature = "stats", test))]
                        {
                            self.last_move_was_see_tested = true;
                        }
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
    pub(crate) const fn last_move_was_see_tested(&self) -> bool {
        self.last_move_was_see_tested
    }

    #[inline]
    #[cfg(any(feature = "stats", test))]
    pub(crate) const fn bad_tactical_count(&self) -> usize {
        self.statistics.bad_capture_count
    }

    #[inline]
    #[cfg(any(feature = "stats", test))]
    pub(crate) fn statistics(&self) -> OrderingStatistics {
        let mut statistics = self.statistics;
        #[cfg(feature = "stats")]
        if self.lazy_tacticals {
            statistics.tactical_candidates_untested = self
                .moves
                .iter()
                .enumerate()
                .filter(|&(index, mv)| {
                    (mv.is_capture() || mv.is_promotion())
                        && matches!(
                            self.classes[index],
                            MoveClass::Unclassified | MoveClass::TacticalCandidate
                        )
                })
                .count() as u64;
        }
        statistics
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

    fn classify_tacticals_exact(&mut self, position: &Position) {
        for (index, &mv) in self.moves.iter().enumerate() {
            if self.classes[index] != MoveClass::Unclassified
                || (!mv.is_capture() && !mv.is_promotion())
            {
                continue;
            }
            let (score, is_good) = tactical_score_exact(position, mv, &mut self.statistics);
            self.scores[index] = score;
            self.classes[index] = if is_good {
                MoveClass::GoodTactical
            } else {
                self.statistics.bad_capture_count += 1;
                MoveClass::BadTactical
            };
        }
    }

    fn classify_tactical_candidates(&mut self, position: &Position) {
        for (index, &mv) in self.moves.iter().enumerate() {
            if self.classes[index] != MoveClass::Unclassified
                || (!mv.is_capture() && !mv.is_promotion())
            {
                continue;
            }
            self.scores[index] = tactical_score_cheap(position, mv, &mut self.statistics);
            self.classes[index] = MoveClass::TacticalCandidate;
        }
    }

    fn pick_next_good_tactical(&mut self, position: &Position) -> Option<Move> {
        loop {
            let index = self.best_index(MoveClass::TacticalCandidate)?;
            let mv = self.moves.as_slice()[index];
            if mv.is_promotion() || threshold_see(position, mv, 0, &mut self.statistics) {
                self.classes[index] = MoveClass::Taken;
                #[cfg(feature = "stats")]
                if mv.is_capture() {
                    self.statistics.good_captures += 1;
                }
                return Some(mv);
            }

            self.classes[index] = MoveClass::BadTactical;
            self.statistics.bad_capture_count += 1;
            #[cfg(feature = "stats")]
            {
                self.statistics.bad_captures += 1;
            }
        }
    }

    fn classify_killers(&mut self, history: &HistoryTable) {
        for (index, &mv) in self.moves.iter().enumerate() {
            if self.classes[index] != MoveClass::Unclassified || !self.killers.contains(&mv) {
                continue;
            }
            self.scores[index] = quiet_score(mv, self.killers, history, self.color);
            self.classes[index] = MoveClass::Killer;
            #[cfg(feature = "stats")]
            {
                self.statistics.moves_scored += 1;
            }
        }
    }

    fn classify_quiets(&mut self, history: &HistoryTable) {
        for (index, &mv) in self.moves.iter().enumerate() {
            if self.classes[index] != MoveClass::Unclassified {
                continue;
            }
            self.scores[index] = quiet_score(mv, self.killers, history, self.color);
            self.classes[index] = MoveClass::Quiet;
            #[cfg(feature = "stats")]
            {
                self.statistics.moves_scored += 1;
            }
        }
    }

    fn pick_best(&mut self, class: MoveClass) -> Option<Move> {
        let index = self.best_index(class)?;
        self.classes[index] = MoveClass::Taken;
        Some(self.moves.as_slice()[index])
    }

    fn best_index(&self, class: MoveClass) -> Option<usize> {
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
        best_index
    }
}

fn tactical_score_exact(
    position: &Position,
    mv: Move,
    _statistics: &mut OrderingStatistics,
) -> (i32, bool) {
    #[cfg(feature = "stats")]
    {
        _statistics.moves_scored += 1;
        _statistics.see_calls += 1;
    }
    #[cfg(feature = "stats")]
    let exchange = {
        let (exchange, work) = see_with_work(position, mv);
        _statistics.see_exchange_steps += work.exchange_steps;
        exchange
    };
    #[cfg(not(feature = "stats"))]
    let exchange = see(position, mv);
    let cheap_score = tactical_score_components(position, mv);
    if mv.is_promotion() || exchange >= 0 {
        #[cfg(feature = "stats")]
        if mv.is_capture() {
            _statistics.good_captures += 1;
        }
        (GOOD_TACTICAL_SCORE + cheap_score + exchange, true)
    } else {
        #[cfg(feature = "stats")]
        {
            _statistics.bad_captures += 1;
        }
        (BAD_CAPTURE_SCORE + cheap_score + exchange, false)
    }
}

fn tactical_score_cheap(
    position: &Position,
    mv: Move,
    _statistics: &mut OrderingStatistics,
) -> i32 {
    #[cfg(feature = "stats")]
    {
        _statistics.moves_scored += 1;
    }
    tactical_score_components(position, mv)
}

fn tactical_score_components(position: &Position, mv: Move) -> i32 {
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
    promotion_bonus + mvv_lva
}

fn threshold_see(
    position: &Position,
    mv: Move,
    threshold: i32,
    _statistics: &mut OrderingStatistics,
) -> bool {
    #[cfg(feature = "stats")]
    {
        _statistics.see_ge_calls += 1;
        let (passes, work) = see_ge_with_work(position, mv, threshold);
        _statistics.see_ge_exchange_steps += work.exchange_steps;
        _statistics.see_ge_early_exits += u64::from(work.early_exit);
        passes
    }
    #[cfg(not(feature = "stats"))]
    {
        see_ge(position, mv, threshold)
    }
}

fn quiet_score(mv: Move, killers: [Move; 2], history: &HistoryTable, color: Color) -> i32 {
    let mut score = 0;
    if mv.is_castle() {
        score += 500;
    }
    if killers[0] == mv {
        score += 90_000;
    } else if killers[1] == mv {
        score += 80_000;
    }
    score + history.score(color, mv)
}

#[cfg(test)]
mod tests {
    use crate::{
        chess::{Move, Position},
        search::see,
    };

    use super::{
        HistoryTable, MovePicker, OrderingStatistics, quiet_score, tactical_score_cheap,
        tactical_score_exact,
    };

    fn collect(picker: &mut MovePicker, position: &Position, history: &HistoryTable) -> Vec<Move> {
        let mut picked = Vec::new();
        while let Some(mv) = picker.next_move(position, history) {
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
        let mut picker = MovePicker::main(
            legal.clone(),
            Some(preferred),
            [killer, Move::NONE],
            position.side_to_move(),
        );

        let picked = collect(&mut picker, &position, &history);

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
            assert_eq!(picker.statistics().see_calls, 0);
            assert_eq!(picker.statistics().see_ge_calls, 3);
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
        let mut picker = MovePicker::main(
            position.legal_moves(),
            Some(preferred),
            [preferred, preferred],
            position.side_to_move(),
        );

        assert_eq!(picker.next_move(&position, &history), Some(preferred));
        assert!(!picker.last_move_was_scored());
        #[cfg(feature = "stats")]
        {
            let statistics = picker.statistics();
            assert_eq!(statistics.moves_scored, 0);
            assert_eq!(statistics.see_calls, 0);
            assert_eq!(statistics.see_ge_calls, 0);
            assert!(statistics.tactical_candidates_untested > 0);
            assert_eq!(statistics.tt_stage_visits, 1);
            assert_eq!(statistics.good_tactical_stage_visits, 0);
        }

        let rest = collect(&mut picker, &position, &history);
        assert!(!rest.contains(&preferred));
        #[cfg(feature = "stats")]
        assert!(picker.statistics().see_ge_calls > 0);
    }

    #[cfg(feature = "stats")]
    #[test]
    fn lazy_main_picker_tests_only_the_candidate_it_is_about_to_return() {
        let mut position = Position::from_fen("6k1/8/5p2/3qp3/2P1Q3/8/8/6K1 w - - 0 1")
            .expect("lazy ordering fixture must be valid");
        let history = HistoryTable::default();
        let mut picker = MovePicker::main(
            position.legal_moves(),
            None,
            [Move::NONE; 2],
            position.side_to_move(),
        );

        let first = picker
            .next_move(&position, &history)
            .expect("fixture has a good tactical move");
        let statistics = picker.statistics();

        assert!(first.is_capture() || first.is_promotion());
        assert!(picker.last_move_was_see_tested());
        assert_eq!(statistics.see_calls, 0);
        assert_eq!(statistics.see_ge_calls, 1);
        assert!(statistics.tactical_candidates_untested > 0);
    }

    #[test]
    fn qsearch_outside_check_returns_only_existing_good_tacticals() {
        let mut position = Position::from_fen("6k1/8/5p2/3qp3/2P1Q3/8/8/6K1 w - - 0 1")
            .expect("ordering fixture must be valid");
        let bad_capture = position
            .find_legal_move("e4e5")
            .expect("losing capture must be legal");
        let history = HistoryTable::default();
        let legal = position.legal_moves();
        let mut expected_statistics = OrderingStatistics::default();
        let mut expected = legal
            .iter()
            .copied()
            .filter(|&mv| {
                (mv.is_capture() || mv.is_promotion())
                    && (mv.is_promotion() || see(&position, mv) >= 0)
            })
            .map(|mv| {
                let score = tactical_score_exact(&position, mv, &mut expected_statistics).0;
                (mv, score)
            })
            .collect::<Vec<_>>();
        expected.sort_by_key(|&(_, score)| std::cmp::Reverse(score));
        let expected = expected.into_iter().map(|(mv, _)| mv).collect::<Vec<_>>();
        let mut picker =
            MovePicker::quiescence(legal, false, [Move::NONE; 2], position.side_to_move());

        let picked = collect(&mut picker, &position, &history);

        assert!(!picked.contains(&bad_capture));
        assert_eq!(picked, expected);
        assert_eq!(picker.bad_tactical_count(), 1);
        assert!(picked.iter().all(|&mv| {
            (mv.is_capture() || mv.is_promotion()) && (mv.is_promotion() || see(&position, mv) >= 0)
        }));
        #[cfg(feature = "stats")]
        {
            assert!(picker.statistics().see_calls > 0);
            assert_eq!(picker.statistics().see_ge_calls, 0);
        }
    }

    #[test]
    fn qsearch_in_check_returns_every_legal_evasion_once() {
        let mut position = Position::from_fen("k5r1/8/8/8/8/7q/4Q1r1/6K1 w - - 0 1")
            .expect("check-evasion fixture must be valid");
        assert!(position.is_in_check(position.side_to_move()));
        let legal = position.legal_moves();
        let history = HistoryTable::default();
        let mut picker = MovePicker::quiescence(
            legal.clone(),
            true,
            [Move::NONE; 2],
            position.side_to_move(),
        );

        let picked = collect(&mut picker, &position, &history);

        assert_eq!(picked.len(), legal.len());
        for &mv in legal.iter() {
            assert_eq!(
                picked.iter().filter(|&&candidate| candidate == mv).count(),
                1
            );
        }
    }

    #[test]
    fn main_picker_preserves_every_legal_move_across_deterministic_play() {
        let mut position = Position::startpos();
        let history = HistoryTable::default();
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
            );

            let picked = collect(&mut picker, &position, &history);

            assert_eq!(picked.len(), legal.len(), "sample {sample}");
            assert_eq!(picked[0], preferred, "sample {sample}");
            let stage_rank = |mv: Move| {
                if mv == preferred {
                    5
                } else if mv.is_capture() || mv.is_promotion() {
                    if mv.is_promotion() || see(&position, mv) >= 0 {
                        4
                    } else {
                        1
                    }
                } else if killers.contains(&mv) {
                    3
                } else {
                    2
                }
            };
            let ranks = picked.iter().copied().map(stage_rank).collect::<Vec<_>>();
            assert!(
                ranks.windows(2).all(|ranks| ranks[0] >= ranks[1]),
                "stage inversion at sample {sample}: {ranks:?}"
            );
            for rank in [4, 1] {
                let mut score_statistics = OrderingStatistics::default();
                let tactical_scores = picked
                    .iter()
                    .copied()
                    .filter(|&mv| stage_rank(mv) == rank)
                    .map(|mv| tactical_score_cheap(&position, mv, &mut score_statistics))
                    .collect::<Vec<_>>();
                assert!(
                    tactical_scores
                        .windows(2)
                        .all(|scores| scores[0] >= scores[1]),
                    "cheap tactical score inversion at sample {sample}: {tactical_scores:?}"
                );
            }
            for rank in [3, 2] {
                let quiet_scores = picked
                    .iter()
                    .copied()
                    .filter(|&mv| stage_rank(mv) == rank)
                    .map(|mv| quiet_score(mv, killers, &history, position.side_to_move()))
                    .collect::<Vec<_>>();
                assert!(
                    quiet_scores.windows(2).all(|scores| scores[0] >= scores[1]),
                    "quiet score inversion at sample {sample}: {quiet_scores:?}"
                );
            }
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
