use crate::chess::{Color, PieceType, Position, Square};

pub(crate) fn evaluate(position: &Position, color: Color) -> (i32, i32) {
    let pawns = position.pieces(color, PieceType::Pawn);
    let enemy_pawns = position.pieces(color.opposite(), PieceType::Pawn);
    let mut middlegame = 0;
    let mut endgame = 0;

    for file in 0..8 {
        let count = (pawns & file_mask(file)).count_ones() as i32;
        if count > 1 {
            middlegame -= (count - 1) * 9;
            endgame -= (count - 1) * 14;
        }
    }

    let mut remaining = pawns;
    while remaining != 0 {
        let index = remaining.trailing_zeros() as u8;
        remaining &= remaining - 1;
        let square = Square::from_index(index).expect("pawn bit is a valid square");
        let file = square.file();
        let adjacent = adjacent_file_mask(file);
        if pawns & adjacent == 0 {
            middlegame -= 10;
            endgame -= 9;
        }
        if enemy_pawns & passed_pawn_mask(square, color) == 0 {
            let advancement = relative_rank(square, color) as i32;
            endgame += advancement * advancement * 6;
        }
    }
    (middlegame, endgame)
}

pub(crate) const fn file_mask(file: u8) -> u64 {
    0x0101_0101_0101_0101_u64 << file
}

fn adjacent_file_mask(file: u8) -> u64 {
    let mut mask = 0_u64;
    if file > 0 {
        mask |= file_mask(file - 1);
    }
    if file < 7 {
        mask |= file_mask(file + 1);
    }
    mask
}

#[inline]
fn passed_pawn_mask(square: Square, color: Color) -> u64 {
    let files = file_mask(square.file()) | adjacent_file_mask(square.file());
    let forward_ranks = match color {
        Color::White if square.rank() < 7 => u64::MAX << ((square.rank() + 1) * 8),
        Color::Black if square.rank() > 0 => (1_u64 << (square.rank() * 8)) - 1,
        Color::White | Color::Black => 0,
    };
    files & forward_ranks
}

const fn relative_rank(square: Square, color: Color) -> u8 {
    match color {
        Color::White => square.rank(),
        Color::Black => 7 - square.rank(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bitboard_passed_pawn_masks_match_the_loop_oracle() {
        for color in [Color::White, Color::Black] {
            for index in 0..64 {
                let square = Square::from_index(index).expect("board index is valid");
                assert_eq!(passed_pawn_mask(square, color), loop_oracle(square, color));
            }
        }
    }

    fn loop_oracle(square: Square, color: Color) -> u64 {
        let (start, end) = match color {
            Color::White => (square.rank() + 1, 8),
            Color::Black => (0, square.rank()),
        };
        let mut mask = 0;
        for rank in start..end {
            for file_delta in -1_i8..=1 {
                let file = square.file() as i8 + file_delta;
                if (0..8).contains(&file)
                    && let Some(target) = Square::from_coords(file as u8, rank)
                {
                    mask |= target.bit();
                }
            }
        }
        mask
    }
}
