use crate::chess::{Color, Move, PieceType, Position, Square};

const COLORS: usize = 2;
const PIECE_TYPES: usize = 6;
const SQUARES: usize = 64;

/// The immediately preceding real move, reduced to the context registered for
/// D1. The piece is the pre-move piece type, including a pawn before promotion.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ContinuationContext {
    color: Color,
    piece: PieceType,
    destination: Square,
}

impl ContinuationContext {
    pub const fn new(color: Color, piece: PieceType, destination: Square) -> Self {
        Self {
            color,
            piece,
            destination,
        }
    }

    pub fn from_move(position: &Position, mv: Move) -> Self {
        let (color, piece) = position
            .piece_at(mv.from())
            .expect("a legal move must start on the moving piece");
        debug_assert_eq!(color, position.side_to_move());
        Self::new(color, piece, mv.to())
    }

    pub const fn color(self) -> Color {
        self.color
    }

    pub const fn piece(self) -> PieceType {
        self.piece
    }

    pub const fn destination(self) -> Square {
        self.destination
    }
}

/// Color-aware first-order continuation history. The current color is implied
/// by normal alternating play, so only the previous moving color is stored.
pub struct ContinuationHistory {
    scores: Box<[i16]>,
}

impl ContinuationHistory {
    pub const ENTRY_COUNT: usize = COLORS * PIECE_TYPES * SQUARES * PIECE_TYPES * SQUARES;
    pub const SCORE_BYTES: usize = Self::ENTRY_COUNT * size_of::<i16>();
    pub const MAX_SCORE: i32 = 16_384;

    #[inline]
    pub fn score(
        &self,
        previous: Option<ContinuationContext>,
        current: ContinuationContext,
    ) -> i32 {
        let Some(previous) = previous else {
            return 0;
        };
        i32::from(self.scores[Self::index(previous, current)])
    }

    pub fn reward(
        &mut self,
        previous: ContinuationContext,
        current: ContinuationContext,
        depth: i32,
    ) {
        self.update(previous, current, history_bonus(depth));
    }

    pub fn penalize(
        &mut self,
        previous: ContinuationContext,
        current: ContinuationContext,
        depth: i32,
    ) {
        self.update(previous, current, -history_bonus(depth));
    }

    pub fn clear(&mut self) {
        self.scores.fill(0);
    }

    #[cfg(any(feature = "stats", test))]
    pub fn distribution(&self) -> ([u64; 7], u64) {
        let mut buckets = [0_u64; 7];
        let mut saturated = 0_u64;
        for &score in self.scores.iter() {
            let score = i32::from(score);
            let bucket = match score {
                -16_384..=-8_193 => 0,
                -8_192..=-2_049 => 1,
                -2_048..=-1 => 2,
                0 => 3,
                1..=2_048 => 4,
                2_049..=8_192 => 5,
                8_193..=16_384 => 6,
                _ => unreachable!("continuation score escaped its registered bounds"),
            };
            buckets[bucket] += 1;
            if score.abs() == Self::MAX_SCORE {
                saturated += 1;
            }
        }
        (buckets, saturated)
    }

    fn update(&mut self, previous: ContinuationContext, current: ContinuationContext, bonus: i32) {
        let entry = &mut self.scores[Self::index(previous, current)];
        let current_score = i32::from(*entry);
        let bonus = bonus.clamp(-Self::MAX_SCORE, Self::MAX_SCORE);
        let updated = current_score + bonus - current_score * bonus.abs() / Self::MAX_SCORE;
        *entry = updated.clamp(-Self::MAX_SCORE, Self::MAX_SCORE) as i16;
    }

    #[inline]
    fn index(previous: ContinuationContext, current: ContinuationContext) -> usize {
        debug_assert_eq!(current.color(), previous.color().opposite());
        ((((previous.color().index() * PIECE_TYPES + previous.piece().index()) * SQUARES
            + previous.destination().index())
            * PIECE_TYPES
            + current.piece().index())
            * SQUARES)
            + current.destination().index()
    }
}

impl Default for ContinuationHistory {
    fn default() -> Self {
        Self {
            scores: vec![0; Self::ENTRY_COUNT].into_boxed_slice(),
        }
    }
}

fn history_bonus(depth: i32) -> i32 {
    (depth.max(1) * depth.max(1) * 16).min(2_048)
}

#[cfg(test)]
mod tests {
    use super::{ContinuationContext, ContinuationHistory};
    use crate::chess::{Color, PieceType, Position, Square};

    fn context(color: Color, piece: PieceType, destination: Square) -> ContinuationContext {
        ContinuationContext::new(color, piece, destination)
    }

    #[test]
    fn storage_has_the_registered_heap_footprint() {
        let history = ContinuationHistory::default();
        assert_eq!(history.scores.len(), 294_912);
        assert_eq!(ContinuationHistory::SCORE_BYTES, 589_824);
        assert_eq!(size_of_val(&*history.scores), 589_824);
    }

    #[test]
    fn key_dimensions_are_isolated() {
        let previous = context(Color::Black, PieceType::Knight, Square::F6);
        let current = context(Color::White, PieceType::Bishop, Square::C4);
        let mut history = ContinuationHistory::default();
        history.reward(previous, current, 8);
        assert!(history.score(Some(previous), current) > 0);

        let alternatives = [
            (
                context(Color::White, PieceType::Knight, Square::F6),
                context(Color::Black, PieceType::Bishop, Square::C4),
            ),
            (context(Color::Black, PieceType::Pawn, Square::F6), current),
            (
                context(Color::Black, PieceType::Knight, Square::E4),
                current,
            ),
            (
                previous,
                context(Color::White, PieceType::Knight, Square::C4),
            ),
            (
                previous,
                context(Color::White, PieceType::Bishop, Square::B5),
            ),
        ];
        for (other_previous, other_current) in alternatives {
            assert_eq!(history.score(Some(other_previous), other_current), 0);
        }
        assert_eq!(history.score(None, current), 0);
    }

    #[test]
    fn real_move_context_uses_the_pre_move_piece_and_destination() {
        let fixtures = [
            (
                "7k/8/8/8/8/8/3p4/K2R4 w - - 0 1",
                "d1d2",
                PieceType::Rook,
                Square::D2,
            ),
            (
                "7k/P7/8/8/8/8/8/K7 w - - 0 1",
                "a7a8q",
                PieceType::Pawn,
                Square::A8,
            ),
            (
                "r3k2r/8/8/8/8/8/8/R3K2R w KQkq - 0 1",
                "e1g1",
                PieceType::King,
                Square::G1,
            ),
            (
                "7k/8/8/3pP3/8/8/8/K7 w - d6 0 1",
                "e5d6",
                PieceType::Pawn,
                Square::D6,
            ),
        ];
        for (fen, notation, piece, destination) in fixtures {
            let mut position = Position::from_fen(fen).expect("context fixture must be valid");
            let mv = position
                .find_legal_move(notation)
                .expect("context move must be legal");
            let actual = ContinuationContext::from_move(&position, mv);
            assert_eq!(actual.color(), Color::White);
            assert_eq!(actual.piece(), piece);
            assert_eq!(actual.destination(), destination);
        }
    }

    #[test]
    fn gravity_is_bounded_and_clear_removes_prior_search_values() {
        let previous = context(Color::Black, PieceType::Pawn, Square::E5);
        let current = context(Color::White, PieceType::Knight, Square::F3);
        let mut history = ContinuationHistory::default();
        for _ in 0..10_000 {
            history.reward(previous, current, 16);
        }
        assert_eq!(
            history.score(Some(previous), current),
            ContinuationHistory::MAX_SCORE
        );
        history.penalize(previous, current, 8);
        assert!(history.score(Some(previous), current) < ContinuationHistory::MAX_SCORE);
        for _ in 0..10_000 {
            history.penalize(previous, current, 16);
        }
        assert_eq!(
            history.score(Some(previous), current),
            -ContinuationHistory::MAX_SCORE
        );
        history.clear();
        assert_eq!(history.score(Some(previous), current), 0);
        assert_eq!(history.distribution(), ([0, 0, 0, 294_912, 0, 0, 0], 0));
    }
}
