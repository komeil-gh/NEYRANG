/// The two players. Array indexing always uses White = 0 and Black = 1.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum Color {
    White = 0,
    Black = 1,
}

impl Color {
    #[inline]
    pub const fn index(self) -> usize {
        self as usize
    }

    #[inline]
    pub const fn opposite(self) -> Self {
        match self {
            Self::White => Self::Black,
            Self::Black => Self::White,
        }
    }
}

/// Piece kinds are laid out in a stable order for bitboard array indexing.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum PieceType {
    Pawn = 0,
    Knight = 1,
    Bishop = 2,
    Rook = 3,
    Queen = 4,
    King = 5,
}

impl PieceType {
    pub const ALL: [Self; 6] = [
        Self::Pawn,
        Self::Knight,
        Self::Bishop,
        Self::Rook,
        Self::Queen,
        Self::King,
    ];

    #[inline]
    pub const fn index(self) -> usize {
        self as usize
    }

    pub(crate) const fn from_fen(byte: u8) -> Option<Self> {
        match byte.to_ascii_lowercase() {
            b'p' => Some(Self::Pawn),
            b'n' => Some(Self::Knight),
            b'b' => Some(Self::Bishop),
            b'r' => Some(Self::Rook),
            b'q' => Some(Self::Queen),
            b'k' => Some(Self::King),
            _ => None,
        }
    }

    pub(crate) const fn fen_byte(self, color: Color) -> u8 {
        let byte = match self {
            Self::Pawn => b'p',
            Self::Knight => b'n',
            Self::Bishop => b'b',
            Self::Rook => b'r',
            Self::Queen => b'q',
            Self::King => b'k',
        };
        match color {
            Color::White => byte.to_ascii_uppercase(),
            Color::Black => byte,
        }
    }
}
