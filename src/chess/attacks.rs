//! Portable attack generation using compile-time leaper and directional-ray tables.

use super::{Bitboard, Color, Square};

const KNIGHT_ATTACKS: [Bitboard; 64] = generate_leaper_table(&[
    (-2, -1),
    (-2, 1),
    (-1, -2),
    (-1, 2),
    (1, -2),
    (1, 2),
    (2, -1),
    (2, 1),
]);
const KING_ATTACKS: [Bitboard; 64] = generate_leaper_table(&[
    (-1, -1),
    (-1, 0),
    (-1, 1),
    (0, -1),
    (0, 1),
    (1, -1),
    (1, 0),
    (1, 1),
]);
const WHITE_PAWN_ATTACKS: [Bitboard; 64] = generate_leaper_table(&[(-1, 1), (1, 1)]);
const BLACK_PAWN_ATTACKS: [Bitboard; 64] = generate_leaper_table(&[(-1, -1), (1, -1)]);

// Each group of four has decreasing square indices first, then increasing ones.
static RAYS: [[Bitboard; 64]; 8] = generate_rays();

const fn generate_rays() -> [[Bitboard; 64]; 8] {
    let directions = [
        (-1, -1),
        (1, -1),
        (-1, 1),
        (1, 1),
        (-1, 0),
        (0, -1),
        (0, 1),
        (1, 0),
    ];
    let mut rays = [[0; 64]; 8];
    let mut direction = 0;
    while direction < 8 {
        let mut square = 0;
        while square < 64 {
            let mut file = (square & 7) as i8 + directions[direction].0;
            let mut rank = (square >> 3) as i8 + directions[direction].1;
            while file >= 0 && file < 8 && rank >= 0 && rank < 8 {
                rays[direction][square] |= 1_u64 << (rank * 8 + file);
                file += directions[direction].0;
                rank += directions[direction].1;
            }
            square += 1;
        }
        direction += 1;
    }
    rays
}

#[inline]
fn directional_attacks<const D: usize>(square: Square, occupancy: Bitboard) -> Bitboard {
    let ray = RAYS[D][square.index()];
    let blockers = ray & occupancy;
    if blockers == 0 {
        return ray;
    }
    let first = if D % 4 < 2 {
        63 - blockers.leading_zeros()
    } else {
        blockers.trailing_zeros()
    } as usize;
    // The ray from the blocker excludes the blocker itself: keep its attack bit.
    ray & !RAYS[D][first]
}

const fn generate_leaper_table(offsets: &[(i8, i8)]) -> [Bitboard; 64] {
    let mut table = [0; 64];
    let mut index = 0_u8;
    while index < 64 {
        let file = (index & 7) as i8;
        let rank = (index >> 3) as i8;
        let mut attacks = 0_u64;
        let mut offset_index = 0;
        while offset_index < offsets.len() {
            let target_file = file + offsets[offset_index].0;
            let target_rank = rank + offsets[offset_index].1;
            if target_file >= 0 && target_file < 8 && target_rank >= 0 && target_rank < 8 {
                attacks |= 1_u64 << (target_rank * 8 + target_file);
            }
            offset_index += 1;
        }
        table[index as usize] = attacks;
        index += 1;
    }
    table
}

#[inline]
pub const fn knight_attacks(square: Square) -> Bitboard {
    KNIGHT_ATTACKS[square.index()]
}

#[inline]
pub const fn king_attacks(square: Square) -> Bitboard {
    KING_ATTACKS[square.index()]
}

#[inline]
pub const fn pawn_attacks(color: Color, square: Square) -> Bitboard {
    match color {
        Color::White => WHITE_PAWN_ATTACKS[square.index()],
        Color::Black => BLACK_PAWN_ATTACKS[square.index()],
    }
}

#[inline]
pub fn bishop_attacks(square: Square, occupancy: Bitboard) -> Bitboard {
    directional_attacks::<0>(square, occupancy)
        | directional_attacks::<1>(square, occupancy)
        | directional_attacks::<2>(square, occupancy)
        | directional_attacks::<3>(square, occupancy)
}

#[inline]
pub fn rook_attacks(square: Square, occupancy: Bitboard) -> Bitboard {
    directional_attacks::<4>(square, occupancy)
        | directional_attacks::<5>(square, occupancy)
        | directional_attacks::<6>(square, occupancy)
        | directional_attacks::<7>(square, occupancy)
}

#[inline]
pub fn queen_attacks(square: Square, occupancy: Bitboard) -> Bitboard {
    bishop_attacks(square, occupancy) | rook_attacks(square, occupancy)
}
