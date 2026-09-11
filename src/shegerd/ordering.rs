use crate::chess::{Color, Move, MoveList, PieceType, Position, Square};

use super::{history::HistoryTable, policy, see::see};

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
    previous_to: Option<Square>,
    policy_enabled: bool,
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
        previous_to: Option<Square>,
        policy_enabled: bool,
    ) -> Self {
        Self::new(
            moves,
            preferred,
            killers,
            color,
            previous_to,
            policy_enabled,
            false,
        )
    }

    pub(crate) fn quiescence(
        moves: MoveList,
        in_check: bool,
        killers: [Move; 2],
        color: Color,
        previous_to: Option<Square>,
    ) -> Self {
        Self::new(
            moves,
            None,
            killers,
            color,
            previous_to,
            !in_check,
            !in_check,
        )
    }

    fn new(
        moves: MoveList,
        preferred: Option<Move>,
        killers: [Move; 2],
        color: Color,
        previous_to: Option<Square>,
        policy_enabled: bool,
        tactical_only: bool,
    ) -> Self {
        Self {
            moves,
            classes: [MoveClass::Unclassified; MoveList::CAPACITY],
            scores: [i32::MIN; MoveList::CAPACITY],
            preferred,
            killers,
            color,
            previous_to,
            policy_enabled,
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
                        self.classify_quiets(position, history);
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
    #[cfg(feature = "stats")]
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
            let (score, is_good) = tactical_score(
                position,
                mv,
                &mut self.statistics,
                self.policy_enabled,
                self.previous_to,
            );
            self.scores[index] = score;
            self.classes[index] = if is_good {
                MoveClass::GoodTactical
            } else {
                self.statistics.bad_capture_count += 1;
                MoveClass::BadTactical
            };
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

    fn classify_quiets(&mut self, position: &Position, history: &HistoryTable) {
        for (index, &mv) in self.moves.iter().enumerate() {
            if self.classes[index] != MoveClass::Unclassified {
                continue;
            }
            self.scores[index] = quiet_score(mv, self.killers, history, self.color)
                + if self.policy_enabled {
                    policy::score(position, mv, self.previous_to, 0)
                } else {
                    0
                };
            self.classes[index] = MoveClass::Quiet;
            #[cfg(feature = "stats")]
            {
                self.statistics.moves_scored += 1;
            }
        }
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
    policy_enabled: bool,
    previous_to: Option<Square>,
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
        let policy_score = if policy_enabled {
            policy::score(position, mv, previous_to, exchange)
        } else {
            0
        };
        (
            GOOD_TACTICAL_SCORE + promotion_bonus + mvv_lva + exchange + policy_score,
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
        shegerd::{policy, see},
    };

    use super::{HistoryTable, MovePicker, OrderingStatistics, quiet_score, tactical_score};

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
            None,
            true,
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
        let mut picker = MovePicker::main(
            position.legal_moves(),
            Some(preferred),
            [preferred, preferred],
            position.side_to_move(),
            None,
            true,
        );

        assert_eq!(picker.next_move(&position, &history), Some(preferred));
        assert!(!picker.last_move_was_scored());
        #[cfg(feature = "stats")]
        {
            let statistics = picker.statistics();
            assert_eq!(statistics.moves_scored, 0);
            assert_eq!(statistics.see_calls, 0);
            assert_eq!(statistics.tt_stage_visits, 1);
            assert_eq!(statistics.good_tactical_stage_visits, 0);
        }

        let rest = collect(&mut picker, &position, &history);
        assert!(!rest.contains(&preferred));
        #[cfg(feature = "stats")]
        assert!(picker.statistics().see_calls > 0);
    }

    #[test]
    fn qsearch_outside_check_returns_only_existing_good_tacticals() {
        let mut position = Position::from_fen("6k1/8/5p2/3qp3/2P1Q3/8/8/6K1 w - - 0 1")
            .expect("ordering fixture must be valid");
        let bad_capture = position
            .find_legal_move("e4e5")
            .expect("losing capture must be legal");
        let history = HistoryTable::default();
        let mut picker = MovePicker::quiescence(
            position.legal_moves(),
            false,
            [Move::NONE; 2],
            position.side_to_move(),
            None,
        );

        let picked = collect(&mut picker, &position, &history);

        assert!(!picked.contains(&bad_capture));
        assert_eq!(picker.bad_tactical_count(), 1);
        assert!(picked.iter().all(|&mv| {
            (mv.is_capture() || mv.is_promotion()) && (mv.is_promotion() || see(&position, mv) >= 0)
        }));
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
            None,
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
    fn policy_changes_only_eligible_stage_ordering() {
        let mut quiet_position = Position::startpos();
        let quiet_moves = quiet_position.legal_moves();
        let history = HistoryTable::default();
        let mut legacy_quiets = MovePicker::main(
            quiet_moves.clone(),
            None,
            [Move::NONE; 2],
            quiet_position.side_to_move(),
            None,
            false,
        );
        let mut policy_quiets = MovePicker::main(
            quiet_moves,
            None,
            [Move::NONE; 2],
            quiet_position.side_to_move(),
            None,
            true,
        );
        assert_ne!(
            collect(&mut legacy_quiets, &quiet_position, &history),
            collect(&mut policy_quiets, &quiet_position, &history)
        );

        let mut tactical_position =
            Position::from_fen("6k1/8/5p2/3qp3/2P1Q3/8/8/6K1 w - - 0 1").unwrap();
        let bad_capture = tactical_position.find_legal_move("e4e5").unwrap();
        let mut statistics = OrderingStatistics::default();
        let legacy = tactical_score(
            &tactical_position,
            bad_capture,
            &mut statistics,
            false,
            None,
        );
        let policy = tactical_score(&tactical_position, bad_capture, &mut statistics, true, None);
        assert!(!legacy.1);
        assert_eq!(legacy, policy, "bad-tactical score must remain unchanged");
        let mut probe = Position::startpos();
        let mut state = 0x5EED_2026_0911_u64;
        let mut reordered_good_tacticals = false;
        for _ in 0..512 {
            let moves = probe.legal_moves();
            if moves.is_empty() {
                probe = Position::startpos();
                continue;
            }
            if !probe.is_in_check(probe.side_to_move()) {
                let mut legacy_tacticals = MovePicker::new(
                    moves.clone(),
                    None,
                    [Move::NONE; 2],
                    probe.side_to_move(),
                    None,
                    false,
                    true,
                );
                let mut policy_tacticals = MovePicker::quiescence(
                    moves.clone(),
                    false,
                    [Move::NONE; 2],
                    probe.side_to_move(),
                    None,
                );
                if collect(&mut legacy_tacticals, &probe, &history)
                    != collect(&mut policy_tacticals, &probe, &history)
                {
                    reordered_good_tacticals = true;
                    break;
                }
            }
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            probe.make_move(moves.as_slice()[(state as usize) % moves.len()]);
        }
        assert!(
            reordered_good_tacticals,
            "policy must reorder at least one deterministic good-tactical set"
        );

        let in_check = "k5r1/8/8/8/8/7q/4Q1r1/6K1 w - - 0 1";
        let mut position = Position::from_fen(in_check).unwrap();
        let moves = position.legal_moves();
        let mut absent = MovePicker::quiescence(
            moves.clone(),
            true,
            [Move::NONE; 2],
            position.side_to_move(),
            None,
        );
        let mut present = MovePicker::quiescence(
            moves,
            true,
            [Move::NONE; 2],
            position.side_to_move(),
            Some(bad_capture.to()),
        );
        assert_eq!(
            collect(&mut absent, &position, &history),
            collect(&mut present, &position, &history),
            "in-check ordering must ignore policy context"
        );
    }

    #[cfg(feature = "stats")]
    #[test]
    fn policy_does_not_add_see_calls() {
        let mut position = Position::from_fen("6k1/8/5p2/3qp3/2P1Q3/8/8/6K1 w - - 0 1").unwrap();
        let moves = position.legal_moves();
        let history = HistoryTable::default();
        let mut legacy = MovePicker::main(
            moves.clone(),
            None,
            [Move::NONE; 2],
            position.side_to_move(),
            None,
            false,
        );
        let mut policy = MovePicker::main(
            moves,
            None,
            [Move::NONE; 2],
            position.side_to_move(),
            None,
            true,
        );
        collect(&mut legacy, &position, &history);
        collect(&mut policy, &position, &history);
        assert_eq!(legacy.statistics().see_calls, policy.statistics().see_calls);
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
                None,
                true,
            );

            let mut repeat = MovePicker::main(
                legal.clone(),
                Some(preferred),
                killers,
                position.side_to_move(),
                None,
                true,
            );
            let picked = collect(&mut picker, &position, &history);

            assert_eq!(picked.len(), legal.len(), "sample {sample}");
            assert_eq!(picked[0], preferred, "sample {sample}");
            assert_eq!(
                picked,
                collect(&mut repeat, &position, &history),
                "sample {sample} must be deterministic"
            );
            let mut score_statistics = OrderingStatistics::default();
            let ordered_scores = picked
                .iter()
                .map(|&mv| {
                    if mv == preferred {
                        1_000_000
                    } else if mv.is_capture() || mv.is_promotion() {
                        tactical_score(&position, mv, &mut score_statistics, true, None).0
                    } else if killers.contains(&mv) {
                        quiet_score(mv, killers, &history, position.side_to_move())
                    } else {
                        quiet_score(mv, killers, &history, position.side_to_move())
                            + policy::score(&position, mv, None, 0)
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
