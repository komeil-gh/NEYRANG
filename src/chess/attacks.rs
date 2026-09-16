//! Portable attack generation using compile-time leaper and directional-ray tables.

#[cfg(all(target_arch = "x86_64", target_feature = "bmi2"))]
use std::sync::LazyLock;

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

#[cfg(all(target_arch = "x86_64", target_feature = "bmi2"))]
const BISHOP_MASKS: [Bitboard; 64] = generate_slider_masks(true);
#[cfg(all(target_arch = "x86_64", target_feature = "bmi2"))]
const ROOK_MASKS: [Bitboard; 64] = generate_slider_masks(false);
#[cfg(all(target_arch = "x86_64", target_feature = "bmi2"))]
const BISHOP_OFFSETS: [usize; 64] = generate_slider_offsets(&BISHOP_MASKS);
#[cfg(all(target_arch = "x86_64", target_feature = "bmi2"))]
const ROOK_OFFSETS: [usize; 64] = generate_slider_offsets(&ROOK_MASKS);
#[cfg(all(target_arch = "x86_64", target_feature = "bmi2"))]
const BISHOP_TABLE_SIZE: usize = slider_table_size(&BISHOP_MASKS, &BISHOP_OFFSETS);
#[cfg(all(target_arch = "x86_64", target_feature = "bmi2"))]
const ROOK_TABLE_SIZE: usize = slider_table_size(&ROOK_MASKS, &ROOK_OFFSETS);

#[cfg(all(target_arch = "x86_64", target_feature = "bmi2"))]
struct SliderTables {
    bishops: Box<[Bitboard]>,
    rooks: Box<[Bitboard]>,
}

#[cfg(all(target_arch = "x86_64", target_feature = "bmi2"))]
static SLIDER_TABLES: LazyLock<SliderTables> = LazyLock::new(build_slider_tables);

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

#[cfg(all(target_arch = "x86_64", target_feature = "bmi2"))]
const fn generate_slider_masks(diagonal: bool) -> [Bitboard; 64] {
    let directions = if diagonal {
        [(-1_i8, -1_i8), (1, -1), (-1, 1), (1, 1)]
    } else {
        [(-1_i8, 0_i8), (0, -1), (0, 1), (1, 0)]
    };
    let mut masks = [0; 64];
    let mut square = 0;
    while square < 64 {
        let mut direction = 0;
        while direction < directions.len() {
            let (file_step, rank_step) = directions[direction];
            let mut file = (square & 7) as i8 + file_step;
            let mut rank = (square >> 3) as i8 + rank_step;
            while file >= 0 && file < 8 && rank >= 0 && rank < 8 {
                let next_file = file + file_step;
                let next_rank = rank + rank_step;
                if next_file < 0 || next_file >= 8 || next_rank < 0 || next_rank >= 8 {
                    break;
                }
                masks[square] |= 1_u64 << (rank * 8 + file);
                file = next_file;
                rank = next_rank;
            }
            direction += 1;
        }
        square += 1;
    }
    masks
}

#[cfg(all(target_arch = "x86_64", target_feature = "bmi2"))]
const fn generate_slider_offsets(masks: &[Bitboard; 64]) -> [usize; 64] {
    let mut offsets = [0; 64];
    let mut square = 1;
    while square < 64 {
        offsets[square] = offsets[square - 1] + (1_usize << masks[square - 1].count_ones());
        square += 1;
    }
    offsets
}

#[cfg(all(target_arch = "x86_64", target_feature = "bmi2"))]
const fn slider_table_size(masks: &[Bitboard; 64], offsets: &[usize; 64]) -> usize {
    offsets[63] + (1_usize << masks[63].count_ones())
}

#[cfg(all(target_arch = "x86_64", target_feature = "bmi2"))]
fn build_slider_tables() -> SliderTables {
    let mut bishops = vec![0; BISHOP_TABLE_SIZE];
    let mut rooks = vec![0; ROOK_TABLE_SIZE];
    for index in 0_u8..64 {
        let square = Square::from_index(index).expect("slider table square is valid");
        fill_slider_table(
            square,
            BISHOP_MASKS[index as usize],
            BISHOP_OFFSETS[index as usize],
            &mut bishops,
            bishop_attacks_ray,
        );
        fill_slider_table(
            square,
            ROOK_MASKS[index as usize],
            ROOK_OFFSETS[index as usize],
            &mut rooks,
            rook_attacks_ray,
        );
    }
    SliderTables {
        bishops: bishops.into_boxed_slice(),
        rooks: rooks.into_boxed_slice(),
    }
}

#[cfg(all(target_arch = "x86_64", target_feature = "bmi2"))]
fn fill_slider_table(
    square: Square,
    mask: Bitboard,
    offset: usize,
    table: &mut [Bitboard],
    attacks: fn(Square, Bitboard) -> Bitboard,
) {
    let entries = 1_usize << mask.count_ones();
    for index in 0..entries {
        table[offset + index] = attacks(square, deposit_bits(index as u64, mask));
    }
}

#[cfg(all(target_arch = "x86_64", target_feature = "bmi2"))]
fn deposit_bits(mut value: u64, mut mask: Bitboard) -> Bitboard {
    let mut result = 0;
    while mask != 0 {
        let bit = mask.isolate_lowest_one();
        if value & 1 != 0 {
            result |= bit;
        }
        value >>= 1;
        mask &= mask - 1;
    }
    result
}

#[cfg(all(target_arch = "x86_64", target_feature = "bmi2"))]
#[inline]
fn slider_index(occupancy: Bitboard, mask: Bitboard, offset: usize) -> usize {
    // SAFETY: this function is compiled only when BMI2 is a target feature.
    offset + unsafe { core::arch::x86_64::_pext_u64(occupancy, mask) as usize }
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
    #[cfg(all(target_arch = "x86_64", target_feature = "bmi2"))]
    {
        let index = square.index();
        SLIDER_TABLES.bishops[slider_index(occupancy, BISHOP_MASKS[index], BISHOP_OFFSETS[index])]
    }
    #[cfg(not(all(target_arch = "x86_64", target_feature = "bmi2")))]
    {
        bishop_attacks_ray(square, occupancy)
    }
}

#[inline]
fn bishop_attacks_ray(square: Square, occupancy: Bitboard) -> Bitboard {
    directional_attacks::<0>(square, occupancy)
        | directional_attacks::<1>(square, occupancy)
        | directional_attacks::<2>(square, occupancy)
        | directional_attacks::<3>(square, occupancy)
}

#[inline]
pub fn rook_attacks(square: Square, occupancy: Bitboard) -> Bitboard {
    #[cfg(all(target_arch = "x86_64", target_feature = "bmi2"))]
    {
        let index = square.index();
        SLIDER_TABLES.rooks[slider_index(occupancy, ROOK_MASKS[index], ROOK_OFFSETS[index])]
    }
    #[cfg(not(all(target_arch = "x86_64", target_feature = "bmi2")))]
    {
        rook_attacks_ray(square, occupancy)
    }
}

#[inline]
fn rook_attacks_ray(square: Square, occupancy: Bitboard) -> Bitboard {
    directional_attacks::<4>(square, occupancy)
        | directional_attacks::<5>(square, occupancy)
        | directional_attacks::<6>(square, occupancy)
        | directional_attacks::<7>(square, occupancy)
}

#[inline]
pub fn queen_attacks(square: Square, occupancy: Bitboard) -> Bitboard {
    bishop_attacks(square, occupancy) | rook_attacks(square, occupancy)
}
