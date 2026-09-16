//! Adapter for Stockfish-compatible HalfKAv2 NNUE networks.

use crate::chess::{Color, PieceType, Position};

impl nnue_rs::Board for Position {
    fn side_to_move(&self) -> nnue_rs::Color {
        map_color(self.side_to_move())
    }

    fn king_square(&self, color: nnue_rs::Color) -> u8 {
        let king = self.pieces(unmap_color(color), PieceType::King);
        debug_assert_eq!(king.count_ones(), 1);
        king.trailing_zeros() as u8
    }

    fn for_each_piece(&self, visitor: &mut dyn FnMut(u8, nnue_rs::Piece)) {
        for square in 0_u8..64 {
            let square =
                crate::chess::Square::from_index(square).expect("a board index is always a square");
            if let Some((color, piece)) = self.piece_at(square) {
                visitor(
                    square.index() as u8,
                    nnue_rs::Piece::new(map_color(color), map_piece(piece)),
                );
            }
        }
    }
}

pub(super) const fn map_color(color: Color) -> nnue_rs::Color {
    match color {
        Color::White => nnue_rs::Color::White,
        Color::Black => nnue_rs::Color::Black,
    }
}

const fn unmap_color(color: nnue_rs::Color) -> Color {
    match color {
        nnue_rs::Color::White => Color::White,
        nnue_rs::Color::Black => Color::Black,
    }
}

const fn map_piece(piece: PieceType) -> nnue_rs::PieceKind {
    match piece {
        PieceType::Pawn => nnue_rs::PieceKind::Pawn,
        PieceType::Knight => nnue_rs::PieceKind::Knight,
        PieceType::Bishop => nnue_rs::PieceKind::Bishop,
        PieceType::Rook => nnue_rs::PieceKind::Rook,
        PieceType::Queen => nnue_rs::PieceKind::Queen,
        PieceType::King => nnue_rs::PieceKind::King,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nnue_rs::Board;

    #[test]
    fn adapter_reports_start_position_exactly() {
        let position = Position::startpos();
        let mut pieces = 0;
        position.for_each_piece(&mut |_, _| pieces += 1);

        assert_eq!(position.side_to_move(), Color::White);
        assert_eq!(Board::king_square(&position, nnue_rs::Color::White), 4);
        assert_eq!(Board::king_square(&position, nnue_rs::Color::Black), 60);
        assert_eq!(pieces, 32);
    }
}
