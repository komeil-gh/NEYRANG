use crate::chess::{Color, Move};

/// Butterfly history indexed by side/from/to. Updates asymptotically approach
/// the bounds, preventing old searches from permanently dominating ordering.
pub struct HistoryTable {
    scores: Box<[[[i16; 64]; 64]; 2]>,
}

impl HistoryTable {
    pub const MAX_SCORE: i32 = 16_384;

    #[inline]
    pub fn score(&self, color: Color, mv: Move) -> i32 {
        i32::from(self.scores[color.index()][mv.from().index()][mv.to().index()])
    }

    pub fn reward(&mut self, color: Color, mv: Move, depth: i32) {
        self.update(color, mv, history_bonus(depth));
    }

    pub fn penalize(&mut self, color: Color, mv: Move, depth: i32) {
        self.update(color, mv, -history_bonus(depth));
    }

    fn update(&mut self, color: Color, mv: Move, bonus: i32) {
        let entry = &mut self.scores[color.index()][mv.from().index()][mv.to().index()];
        let current = i32::from(*entry);
        let bonus = bonus.clamp(-Self::MAX_SCORE, Self::MAX_SCORE);
        let updated = current + bonus - current * bonus.abs() / Self::MAX_SCORE;
        *entry = updated.clamp(-Self::MAX_SCORE, Self::MAX_SCORE) as i16;
    }
}

impl Default for HistoryTable {
    fn default() -> Self {
        Self {
            scores: Box::new([[[0; 64]; 64]; 2]),
        }
    }
}

fn history_bonus(depth: i32) -> i32 {
    (depth.max(1) * depth.max(1) * 16).min(2_048)
}
