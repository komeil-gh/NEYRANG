use crate::chess::{Move, PieceType, Position};

const COLORS: usize = 2;
const PIECE_TYPES: usize = 6;
const SQUARES: usize = 64;

/// Capture-cutoff history keyed by colored moving piece, destination, and
/// captured piece type. One table belongs to one search worker.
pub struct CaptureHistoryTable {
    scores: Box<[i16; Self::ENTRY_COUNT]>,
}

impl CaptureHistoryTable {
    pub const ENTRY_COUNT: usize = COLORS * PIECE_TYPES * SQUARES * PIECE_TYPES;
    pub const STORAGE_BYTES: usize = Self::ENTRY_COUNT * size_of::<i16>();
    pub const MAX_SCORE: i32 = 8_192;

    #[inline]
    pub fn score(&self, position: &Position, mv: Move) -> i32 {
        self.key(position, mv)
            .map_or(0, |index| i32::from(self.scores[index]))
    }

    pub fn reward(&mut self, position: &Position, mv: Move, depth: i32) -> bool {
        self.update(position, mv, capture_history_bonus(depth))
    }

    pub fn penalize(&mut self, position: &Position, mv: Move, depth: i32) -> bool {
        self.update(position, mv, -capture_history_bonus(depth))
    }

    fn update(&mut self, position: &Position, mv: Move, bonus: i32) -> bool {
        let Some(index) = self.key(position, mv) else {
            return false;
        };
        let entry = &mut self.scores[index];
        let current = i32::from(*entry);
        let bonus = bonus.clamp(-Self::MAX_SCORE, Self::MAX_SCORE);
        let updated = current + bonus - current * bonus.abs() / Self::MAX_SCORE;
        *entry = updated.clamp(-Self::MAX_SCORE, Self::MAX_SCORE) as i16;
        true
    }

    #[inline]
    fn key(&self, position: &Position, mv: Move) -> Option<usize> {
        if !mv.is_capture() {
            return None;
        }
        let (color, moving_piece) = position.piece_at(mv.from())?;
        let captured_piece = if mv.is_en_passant() {
            PieceType::Pawn
        } else {
            position.piece_at(mv.to())?.1
        };
        Some(
            (((color.index() * PIECE_TYPES + moving_piece.index()) * SQUARES + mv.to().index())
                * PIECE_TYPES)
                + captured_piece.index(),
        )
    }

    #[cfg(feature = "stats")]
    pub(crate) fn summary(&self) -> CaptureHistorySummary {
        let mut summary = CaptureHistorySummary::default();
        for &entry in self.scores.iter() {
            let score = i32::from(entry);
            let bucket = match score {
                -8_192..=-4_097 => 0,
                -4_096..=-1_025 => 1,
                -1_024..=-1 => 2,
                0 => 3,
                1..=1_024 => 4,
                1_025..=4_096 => 5,
                4_097..=8_192 => 6,
                _ => unreachable!("capture history score escaped its registered bounds"),
            };
            summary.buckets[bucket] += 1;
            if score.abs() == Self::MAX_SCORE {
                summary.saturated += 1;
            }
        }
        summary
    }
}

impl Default for CaptureHistoryTable {
    fn default() -> Self {
        Self {
            scores: Box::new([0; Self::ENTRY_COUNT]),
        }
    }
}

#[cfg(feature = "stats")]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct CaptureHistorySummary {
    pub buckets: [u64; 7],
    pub saturated: u64,
}

fn capture_history_bonus(depth: i32) -> i32 {
    let depth = depth.max(1);
    (depth * depth * 32).min(2_048)
}

#[cfg(test)]
mod tests {
    use crate::chess::Position;

    use super::CaptureHistoryTable;

    fn position_and_move(fen: &str, notation: &str) -> (Position, crate::chess::Move) {
        let mut position = Position::from_fen(fen).expect("capture-history fixture must be valid");
        let mv = position
            .find_legal_move(notation)
            .expect("capture-history move must be legal");
        (position, mv)
    }

    #[test]
    fn registered_table_has_exact_storage_footprint() {
        assert_eq!(CaptureHistoryTable::ENTRY_COUNT, 4_608);
        assert_eq!(CaptureHistoryTable::STORAGE_BYTES, 9_216);
    }

    #[test]
    fn key_isolates_color_mover_destination_and_victim() {
        let mut history = CaptureHistoryTable::default();
        let (white, white_knight_d5) =
            position_and_move("7k/8/8/1p1p4/8/2N5/8/K7 w - - 0 1", "c3d5");
        let white_knight_b5 = white
            .clone()
            .find_legal_move("c3b5")
            .expect("destination-isolation move must be legal");
        let (black, black_knight_d5) = position_and_move("7k/2n5/8/3P4/8/8/8/K7 b - - 0 1", "c7d5");
        let (bishop, white_bishop_d5) =
            position_and_move("7k/8/8/3p4/8/1B6/8/K7 w - - 0 1", "b3d5");
        let (queen_victim, knight_takes_queen) =
            position_and_move("7k/8/8/3q4/8/2N5/8/K7 w - - 0 1", "c3d5");

        assert!(history.reward(&white, white_knight_d5, 6));
        assert!(history.score(&white, white_knight_d5) > 0);
        assert_eq!(history.score(&white, white_knight_b5), 0);
        assert_eq!(history.score(&black, black_knight_d5), 0);
        assert_eq!(history.score(&bishop, white_bishop_d5), 0);
        assert_eq!(history.score(&queen_victim, knight_takes_queen), 0);
    }

    #[test]
    fn en_passant_and_capture_promotion_are_indexed_but_quiet_promotion_is_not() {
        let mut history = CaptureHistoryTable::default();
        let (en_passant, ep) = position_and_move("7k/8/8/3pP3/8/8/8/K7 w - d6 0 1", "e5d6");
        let (promotion, capture_promotion) =
            position_and_move("k6r/6P1/8/8/8/8/8/K7 w - - 0 1", "g7h8q");
        let quiet_promotion = promotion
            .clone()
            .find_legal_move("g7g8q")
            .expect("quiet promotion must be legal");

        assert!(history.reward(&en_passant, ep, 4));
        assert!(history.reward(&promotion, capture_promotion, 4));
        assert!(!history.reward(&promotion, quiet_promotion, 4));
        assert!(history.score(&en_passant, ep) > 0);
        assert!(history.score(&promotion, capture_promotion) > 0);
        assert_eq!(history.score(&promotion, quiet_promotion), 0);
    }

    #[test]
    fn gravity_is_bounded_and_malus_reverses_a_reward() {
        let mut history = CaptureHistoryTable::default();
        let (position, mv) = position_and_move("7k/8/8/3p4/8/2N5/8/K7 w - - 0 1", "c3d5");

        for _ in 0..10_000 {
            assert!(history.reward(&position, mv, 8));
        }
        let rewarded = history.score(&position, mv);
        assert!(rewarded > 0);
        assert!(rewarded <= CaptureHistoryTable::MAX_SCORE);

        assert!(history.penalize(&position, mv, 8));
        assert!(history.score(&position, mv) < rewarded);
        for _ in 0..20_000 {
            assert!(history.penalize(&position, mv, 8));
        }
        assert!(history.score(&position, mv) >= -CaptureHistoryTable::MAX_SCORE);
    }
}
