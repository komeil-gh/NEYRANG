use std::fmt;

use super::{PieceType, Square};

/// The high four bits of a move encode mutually exclusive chess semantics.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[repr(u16)]
pub enum MoveFlag {
    #[default]
    Quiet = 0,
    DoublePawnPush = 1,
    KingCastle = 2,
    QueenCastle = 3,
    Capture = 4,
    EnPassant = 5,
    KnightPromotion = 8,
    BishopPromotion = 9,
    RookPromotion = 10,
    QueenPromotion = 11,
    KnightPromotionCapture = 12,
    BishopPromotionCapture = 13,
    RookPromotionCapture = 14,
    QueenPromotionCapture = 15,
}

impl MoveFlag {
    const fn from_bits(bits: u16) -> Self {
        match bits {
            0 => Self::Quiet,
            1 => Self::DoublePawnPush,
            2 => Self::KingCastle,
            3 => Self::QueenCastle,
            4 => Self::Capture,
            5 => Self::EnPassant,
            8 => Self::KnightPromotion,
            9 => Self::BishopPromotion,
            10 => Self::RookPromotion,
            11 => Self::QueenPromotion,
            12 => Self::KnightPromotionCapture,
            13 => Self::BishopPromotionCapture,
            14 => Self::RookPromotionCapture,
            15 => Self::QueenPromotionCapture,
            _ => Self::Quiet,
        }
    }
}

/// Packed 16-bit move: from (6), to (6), semantic flag (4).
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[repr(transparent)]
pub struct Move(u16);

impl Move {
    pub const NONE: Self = Self(u16::MAX);

    #[inline]
    pub(crate) const fn from_raw(raw: u16) -> Self {
        Self(raw)
    }

    #[inline]
    pub const fn new(from: Square, to: Square, flag: MoveFlag) -> Self {
        Self(from.index() as u16 | ((to.index() as u16) << 6) | ((flag as u16) << 12))
    }

    #[inline]
    pub fn from(self) -> Square {
        Square::from_index((self.0 & 0x3f) as u8).expect("encoded source square is in range")
    }

    #[inline]
    pub fn to(self) -> Square {
        Square::from_index(((self.0 >> 6) & 0x3f) as u8)
            .expect("encoded destination square is in range")
    }

    #[inline]
    pub const fn flag(self) -> MoveFlag {
        MoveFlag::from_bits(self.0 >> 12)
    }

    #[inline]
    pub const fn raw(self) -> u16 {
        self.0
    }

    #[inline]
    pub const fn is_capture(self) -> bool {
        matches!(
            self.flag(),
            MoveFlag::Capture
                | MoveFlag::EnPassant
                | MoveFlag::KnightPromotionCapture
                | MoveFlag::BishopPromotionCapture
                | MoveFlag::RookPromotionCapture
                | MoveFlag::QueenPromotionCapture
        )
    }

    #[inline]
    pub const fn promotion(self) -> Option<PieceType> {
        match self.flag() {
            MoveFlag::KnightPromotion | MoveFlag::KnightPromotionCapture => Some(PieceType::Knight),
            MoveFlag::BishopPromotion | MoveFlag::BishopPromotionCapture => Some(PieceType::Bishop),
            MoveFlag::RookPromotion | MoveFlag::RookPromotionCapture => Some(PieceType::Rook),
            MoveFlag::QueenPromotion | MoveFlag::QueenPromotionCapture => Some(PieceType::Queen),
            _ => None,
        }
    }

    #[inline]
    pub const fn is_promotion(self) -> bool {
        self.promotion().is_some()
    }

    #[inline]
    pub const fn is_castle(self) -> bool {
        matches!(self.flag(), MoveFlag::KingCastle | MoveFlag::QueenCastle)
    }

    #[inline]
    pub const fn is_en_passant(self) -> bool {
        matches!(self.flag(), MoveFlag::EnPassant)
    }

    #[inline]
    pub const fn is_double_pawn_push(self) -> bool {
        matches!(self.flag(), MoveFlag::DoublePawnPush)
    }
}

impl fmt::Display for Move {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}{}", self.from(), self.to())?;
        if let Some(piece) = self.promotion() {
            formatter.write_str(match piece {
                PieceType::Knight => "n",
                PieceType::Bishop => "b",
                PieceType::Rook => "r",
                PieceType::Queen => "q",
                PieceType::Pawn | PieceType::King => unreachable!("invalid promotion type"),
            })?;
        }
        Ok(())
    }
}

/// Stack-friendly legal/pseudo-legal move buffer with chess-safe capacity.
#[derive(Clone)]
pub struct MoveList {
    moves: [Move; Self::CAPACITY],
    len: usize,
}

impl MoveList {
    pub const CAPACITY: usize = 256;

    pub const fn new() -> Self {
        Self {
            moves: [Move::NONE; Self::CAPACITY],
            len: 0,
        }
    }

    #[inline]
    pub fn push(&mut self, mv: Move) {
        assert!(
            self.len < Self::CAPACITY,
            "legal move list exceeded 256 moves"
        );
        self.moves[self.len] = mv;
        self.len += 1;
    }

    #[inline]
    pub const fn len(&self) -> usize {
        self.len
    }

    #[inline]
    pub fn iter(&self) -> std::slice::Iter<'_, Move> {
        self.moves[..self.len].iter()
    }

    #[inline]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    #[inline]
    pub fn as_slice(&self) -> &[Move] {
        &self.moves[..self.len]
    }

    #[inline]
    pub fn as_mut_slice(&mut self) -> &mut [Move] {
        &mut self.moves[..self.len]
    }
}

impl Default for MoveList {
    fn default() -> Self {
        Self::new()
    }
}
