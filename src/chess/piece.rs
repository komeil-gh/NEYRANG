use super::{Color, PieceType};

/// A colored piece stored in the mailbox representation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Piece {
    pub color: Color,
    pub kind: PieceType,
}

impl Piece {
    pub(crate) const fn from_fen(byte: u8) -> Option<Self> {
        let kind = match PieceType::from_fen(byte) {
            Some(kind) => kind,
            None => return None,
        };
        let color = if byte.is_ascii_uppercase() {
            Color::White
        } else {
            Color::Black
        };
        Some(Self { color, kind })
    }
}
