//! Deterministic Zobrist keys generated from a fixed SplitMix64 seed.

use super::{Color, Piece, PieceType, Position, Square};

const SEED: u64 = 0xA4D7_91E5_C36B_2F08;
const SIDE_INDEX: u64 = 768;
const CASTLING_INDEX: u64 = 769;
const EN_PASSANT_INDEX: u64 = 785;

const fn splitmix64(index: u64) -> u64 {
    let mut value = SEED.wrapping_add(index.wrapping_mul(0x9E37_79B9_7F4A_7C15));
    value = (value ^ (value >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    value ^ (value >> 31)
}

#[inline]
pub(crate) const fn piece_key(piece: Piece, square: Square) -> u64 {
    let index =
        piece.color.index() as u64 * 384 + piece.kind.index() as u64 * 64 + square.index() as u64;
    splitmix64(index)
}

#[inline]
pub(crate) const fn side_key() -> u64 {
    splitmix64(SIDE_INDEX)
}

#[inline]
pub(crate) const fn castling_key(rights: u8) -> u64 {
    splitmix64(CASTLING_INDEX + rights as u64)
}

#[inline]
pub(crate) const fn en_passant_key(file: u8) -> u64 {
    splitmix64(EN_PASSANT_INDEX + file as u64)
}

pub fn recompute(position: &Position) -> u64 {
    let mut hash = 0_u64;
    for color in [Color::White, Color::Black] {
        for kind in PieceType::ALL {
            let mut pieces = position.pieces(color, kind);
            while pieces != 0 {
                let index = pieces.trailing_zeros() as u8;
                pieces &= pieces - 1;
                let square = Square::from_index(index).expect("set bit is a valid square");
                hash ^= piece_key(Piece { color, kind }, square);
            }
        }
    }
    if position.side_to_move() == Color::Black {
        hash ^= side_key();
    }
    hash ^= castling_key(position.castling_rights().bits());
    if let Some(square) = position.en_passant() {
        hash ^= en_passant_key(square.file());
    }
    hash
}
