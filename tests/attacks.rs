use neyrang::chess::{Square, attacks};

// Coordinate stepping is deliberately independent of production lookup masks.
fn slow_attacks(square: Square, occupancy: u64, diagonal: bool) -> u64 {
    let mut result = 0;
    for file_step in -1_i8..=1 {
        for rank_step in -1_i8..=1 {
            if (file_step == 0 && rank_step == 0)
                || ((file_step != 0 && rank_step != 0) != diagonal)
            {
                continue;
            }
            for distance in 1..8 {
                let file = square.file() as i8 + file_step * distance;
                let rank = square.rank() as i8 + rank_step * distance;
                if !(0..8).contains(&file) || !(0..8).contains(&rank) {
                    break;
                }
                let bit = 1_u64 << (rank * 8 + file);
                result |= bit;
                if occupancy & bit != 0 {
                    break;
                }
            }
        }
    }
    result
}

#[test]
fn slider_masks_match_all_ray_occupancies() {
    let mut subsets = 0;
    for index in 0..64 {
        let square = Square::from_index(index).unwrap();
        for diagonal in [false, true] {
            let mask = slow_attacks(square, 0, diagonal);
            let mut occupancy = 0_u64;
            loop {
                let expected = slow_attacks(square, occupancy, diagonal);
                // Includes source-square occupancy and every irrelevant bit.
                for actual_occupancy in [occupancy, occupancy | !mask] {
                    let actual = if diagonal {
                        attacks::bishop_attacks(square, actual_occupancy)
                    } else {
                        attacks::rook_attacks(square, actual_occupancy)
                    };
                    assert_eq!(
                        actual, expected,
                        "{square} {diagonal} {actual_occupancy:016x}"
                    );
                    assert_eq!(
                        attacks::queen_attacks(square, actual_occupancy),
                        slow_attacks(square, actual_occupancy, false)
                            | slow_attacks(square, actual_occupancy, true),
                        "queen {square} {actual_occupancy:016x}"
                    );
                }
                subsets += 1;
                occupancy = occupancy.wrapping_sub(mask) & mask;
                if occupancy == 0 {
                    break;
                }
            }
        }
    }
    assert_eq!(subsets, 1_119_744);
}
