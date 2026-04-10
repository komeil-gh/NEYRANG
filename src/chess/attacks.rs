//! Portable attack generation. Leaper tables are built at compile time;
//! sliders use straightforward occupancy rays until profiling justifies tables.

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
    ray_attacks(square, occupancy, &[(-1, -1), (-1, 1), (1, -1), (1, 1)])
}

#[inline]
pub fn rook_attacks(square: Square, occupancy: Bitboard) -> Bitboard {
    ray_attacks(square, occupancy, &[(-1, 0), (0, -1), (0, 1), (1, 0)])
}

#[inline]
pub fn queen_attacks(square: Square, occupancy: Bitboard) -> Bitboard {
    bishop_attacks(square, occupancy) | rook_attacks(square, occupancy)
}

fn ray_attacks(square: Square, occupancy: Bitboard, directions: &[(i8, i8)]) -> Bitboard {
    let mut attacks = 0_u64;
    for &(file_step, rank_step) in directions {
        let mut file = square.file() as i8 + file_step;
        let mut rank = square.rank() as i8 + rank_step;
        while (0..8).contains(&file) && (0..8).contains(&rank) {
            let target = Square::from_coords(file as u8, rank as u8)
                .expect("validated ray coordinates are on the board");
            attacks |= target.bit();
            if occupancy & target.bit() != 0 {
                break;
            }
            file += file_step;
            rank += rank_step;
        }
    }
    attacks
}
