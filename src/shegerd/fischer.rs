//! Frozen Fischer-derived root-ordering preference.

use crate::chess::{Color, Move, MoveList, PieceType, Position};

const MAGIC: &[u8; 8] = b"NYRFSP1\0";
const VERSION: u32 = 1;
const PHASES: usize = 3;
const PIECES: usize = 6;
const SQUARES: usize = 64;
const FROM_TO_LEN: usize = PHASES * PIECES * SQUARES * SQUARES;
const PIECE_TO_LEN: usize = PHASES * PIECES * SQUARES;
const HEADER_LEN: usize = 32;
const MODEL_LEN: usize = HEADER_LEN + 2 * (FROM_TO_LEN + PIECE_TO_LEN);
const MODEL: &[u8; MODEL_LEN] = include_bytes!("../../assets/models/fischer-prior-v1.bin");

fn u32_at(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(
        bytes[offset..offset + 4]
            .try_into()
            .expect("four-byte field"),
    )
}

fn valid_model(bytes: &[u8]) -> bool {
    bytes.len() == MODEL_LEN
        && &bytes[..8] == MAGIC
        && u32_at(bytes, 8) == VERSION
        && u32_at(bytes, 12) as usize == PHASES
        && u32_at(bytes, 16) as usize == PIECES
        && u32_at(bytes, 20) as usize == SQUARES
        && u32_at(bytes, 24) as usize == FROM_TO_LEN
        && u32_at(bytes, 28) as usize == PIECE_TO_LEN
}

fn material_phase(position: &Position) -> usize {
    let mut material = 0_u32;
    for color in [Color::White, Color::Black] {
        for (piece, value) in [
            (PieceType::Knight, 3),
            (PieceType::Bishop, 3),
            (PieceType::Rook, 5),
            (PieceType::Queen, 9),
        ] {
            material += position.pieces(color, piece).count_ones() * value;
        }
    }
    if material >= 52 {
        0
    } else if material >= 24 {
        1
    } else {
        2
    }
}

fn normalized_square(index: usize, side: Color) -> usize {
    if side == Color::White {
        index
    } else {
        index ^ 56
    }
}

fn weight(index: usize) -> i32 {
    let offset = HEADER_LEN + 2 * index;
    i16::from_le_bytes([MODEL[offset], MODEL[offset + 1]]) as i32
}

pub(crate) fn score(position: &Position, mv: Move) -> i32 {
    assert!(valid_model(MODEL), "embedded Fischer prior is invalid");
    let side = position.side_to_move();
    let piece = position
        .piece_at(mv.from())
        .expect("a legal move must have a source piece")
        .1
        .index();
    let from = normalized_square(mv.from().index(), side);
    let to = normalized_square(mv.to().index(), side);
    let phase = material_phase(position);
    let from_to = (((phase * PIECES + piece) * SQUARES + from) * SQUARES) + to;
    let piece_to = ((phase * PIECES + piece) * SQUARES) + to;
    weight(from_to) + weight(FROM_TO_LEN + piece_to)
}

fn uci_key(mv: Move) -> (u8, u8, u8, u8, u8) {
    let promotion = match mv.promotion() {
        None => 0,
        Some(PieceType::Bishop) => b'b',
        Some(PieceType::Knight) => b'n',
        Some(PieceType::Queen) => b'q',
        Some(PieceType::Rook) => b'r',
        Some(PieceType::Pawn | PieceType::King) => unreachable!("invalid promotion"),
    };
    (
        mv.from().file(),
        mv.from().rank(),
        mv.to().file(),
        mv.to().rank(),
        promotion,
    )
}

pub(crate) fn preferred(position: &Position, moves: &MoveList) -> Option<Move> {
    let game_ply = (u32::from(position.fullmove_number()) - 1) * 2
        + u32::from(position.side_to_move() == Color::Black);
    if game_ply < 12 {
        return None;
    }
    let mut moves = moves.iter().copied();
    let mut best = moves.next()?;
    let mut best_score = score(position, best);
    for mv in moves {
        let candidate_score = score(position, mv);
        if candidate_score > best_score
            || (candidate_score == best_score && uci_key(mv) < uci_key(best))
        {
            best = mv;
            best_score = candidate_score;
        }
    }
    Some(best)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn move_named(position: &Position, name: &str) -> Move {
        position
            .clone()
            .legal_moves()
            .iter()
            .copied()
            .find(|mv| mv.to_string() == name)
            .expect("fixture move must be legal")
    }

    #[test]
    fn embedded_model_matches_the_frozen_contract() {
        assert!(valid_model(MODEL));
    }

    #[test]
    fn preferred_move_is_legal_and_deterministic() {
        let mut start = Position::startpos();
        let start_moves = start.legal_moves();
        assert_eq!(None, preferred(&start, &start_moves));
        let mut position =
            Position::from_fen("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 7")
                .expect("fixture FEN");
        let moves = position.legal_moves();
        let first = preferred(&position, &moves).expect("startpos has legal moves");
        assert!(moves.iter().any(|&mv| mv == first));
        assert_eq!(Some(first), preferred(&position, &moves));
    }

    #[test]
    fn scores_match_the_python_fitter_fixtures() {
        let white = Position::from_fen("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 7")
            .expect("white fixture FEN");
        assert_eq!(447, score(&white, move_named(&white, "e2e4")));
        assert_eq!(807, score(&white, move_named(&white, "d2d4")));
        assert_eq!(707, score(&white, move_named(&white, "g1f3")));

        let black =
            Position::from_fen("rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq - 0 7")
                .expect("black fixture FEN");
        assert_eq!(161, score(&black, move_named(&black, "c7c5")));
        assert_eq!(447, score(&black, move_named(&black, "e7e5")));

        let endgame =
            Position::from_fen("8/8/8/3k4/8/8/4K3/7R w - - 0 7").expect("endgame fixture FEN");
        assert_eq!(-54, score(&endgame, move_named(&endgame, "h1h5")));
        assert_eq!(273, score(&endgame, move_named(&endgame, "e2e3")));
    }
}
