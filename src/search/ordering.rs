use crate::chess::{Color, Move, MoveList, PieceType, Position};

use super::history::HistoryTable;

const PIECE_VALUE: [i32; 6] = [100, 320, 330, 500, 900, 20_000];

pub(crate) fn order(
    position: &Position,
    moves: &mut MoveList,
    preferred: Option<Move>,
    killers: [Move; 2],
    history: &HistoryTable,
    color: Color,
) {
    moves.as_mut_slice().sort_unstable_by_key(|&mv| {
        std::cmp::Reverse(score(position, mv, preferred, killers, history, color))
    });
}

fn score(
    position: &Position,
    mv: Move,
    preferred: Option<Move>,
    killers: [Move; 2],
    history: &HistoryTable,
    color: Color,
) -> i32 {
    if preferred == Some(mv) {
        return 1_000_000;
    }
    let mut score = 0;
    if let Some(promotion) = mv.promotion() {
        score += 500_000 + PIECE_VALUE[promotion.index()];
    }
    if mv.is_capture() {
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
        score += 100_000 + PIECE_VALUE[victim.index()] * 16 - PIECE_VALUE[attacker.index()];
    }
    if mv.is_castle() {
        score += 500;
    }
    if !mv.is_capture() && !mv.is_promotion() {
        if killers[0] == mv {
            score += 90_000;
        } else if killers[1] == mv {
            score += 80_000;
        }
        score += history.score(color, mv);
    }
    score
}
