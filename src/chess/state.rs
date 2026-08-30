use super::{CastlingRights, Piece, Square};

/// Search-only state saved while temporarily passing the side to move.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NullUndoState {
    en_passant: Option<Square>,
    hash: u64,
}

impl NullUndoState {
    pub(crate) const fn new(en_passant: Option<Square>, hash: u64) -> Self {
        Self { en_passant, hash }
    }

    pub(crate) const fn en_passant(self) -> Option<Square> {
        self.en_passant
    }

    pub(crate) const fn hash(self) -> u64 {
        self.hash
    }
}

/// Irreversible information saved once per made move.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UndoState {
    captured: Option<(Square, Piece)>,
    castling_rights: CastlingRights,
    en_passant: Option<Square>,
    halfmove_clock: u16,
    fullmove_number: u16,
    hash: u64,
}

impl UndoState {
    pub(crate) const fn new(
        captured: Option<(Square, Piece)>,
        castling_rights: CastlingRights,
        en_passant: Option<Square>,
        halfmove_clock: u16,
        fullmove_number: u16,
        hash: u64,
    ) -> Self {
        Self {
            captured,
            castling_rights,
            en_passant,
            halfmove_clock,
            fullmove_number,
            hash,
        }
    }

    pub(crate) const fn captured(self) -> Option<(Square, Piece)> {
        self.captured
    }

    pub(crate) const fn castling_rights(self) -> CastlingRights {
        self.castling_rights
    }

    pub(crate) const fn en_passant(self) -> Option<Square> {
        self.en_passant
    }

    pub(crate) const fn halfmove_clock(self) -> u16 {
        self.halfmove_clock
    }

    pub(crate) const fn fullmove_number(self) -> u16 {
        self.fullmove_number
    }

    pub(crate) const fn hash(self) -> u64 {
        self.hash
    }
}
