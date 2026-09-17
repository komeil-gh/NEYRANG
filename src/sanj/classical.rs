use crate::chess::{Color, PieceType, Position, Square, attacks};

use super::{pawns, psqt};

pub const TEMPO: i32 = 6;

const MG_VALUE: [i32; 6] = [69, 300, 312, 405, 1_094, 0];
const EG_VALUE: [i32; 6] = [79, 280, 330, 589, 1_077, 0];
const PHASE_WEIGHT: [i32; 6] = [0, 1, 1, 2, 4, 0];
const MAX_PHASE: i32 = 24;

/// Return centipawns from the side-to-move perspective.
pub fn evaluate(position: &Position) -> i32 {
    let white = color_terms(position, Color::White);
    let black = color_terms(position, Color::Black);
    let mut middlegame = white.middlegame - black.middlegame;
    let endgame = white.endgame - black.endgame;
    let mut phase = white.phase + black.phase;
    middlegame += king_safety(position, Color::White, black.king_pressure);
    middlegame -= king_safety(position, Color::Black, white.king_pressure);

    phase = phase.clamp(0, MAX_PHASE);
    let white_score = (middlegame * phase + endgame * (MAX_PHASE - phase)) / MAX_PHASE;
    match position.side_to_move() {
        Color::White => white_score + TEMPO,
        Color::Black => -white_score + TEMPO,
    }
}

#[derive(Clone, Copy)]
struct ColorTerms {
    middlegame: i32,
    endgame: i32,
    phase: i32,
    king_pressure: i32,
}

fn color_terms(position: &Position, color: Color) -> ColorTerms {
    let own = position.occupancy(color);
    let all = position.all_occupancy();
    let own_pawns = position.pieces(color, PieceType::Pawn);
    let all_pawns = own_pawns | position.pieces(color.opposite(), PieceType::Pawn);
    let enemy_king = position.pieces(color.opposite(), PieceType::King);
    let zone = if enemy_king == 0 {
        0
    } else {
        let square =
            Square::from_index(enemy_king.trailing_zeros() as u8).expect("king bit is valid");
        attacks::king_attacks(square) | square.bit()
    };
    let mut middlegame = 0;
    let mut endgame = 0;
    let mut phase = 0;
    let mut pressure = 0;
    let mut attackers = 0;
    let mut bishop_count = 0;
    let mut has_queen = false;
    let mut rook_file_score = 0;
    for kind in PieceType::ALL {
        let mut pieces = position.pieces(color, kind);
        let count = pieces.count_ones() as i32;
        phase += count * PHASE_WEIGHT[kind.index()];
        if kind == PieceType::Bishop {
            bishop_count = count;
        } else if kind == PieceType::Queen {
            has_queen = count != 0;
        }
        while pieces != 0 {
            let index = pieces.trailing_zeros() as u8;
            pieces &= pieces - 1;
            let square = Square::from_index(index).expect("piece bit is a valid square");
            middlegame += MG_VALUE[kind.index()] + psqt::middlegame(kind, square, color);
            endgame += EG_VALUE[kind.index()] + psqt::endgame(kind, square, color);

            if kind == PieceType::Pawn {
                pressure += (attacks::pawn_attacks(color, square) & zone).count_ones() as i32;
                continue;
            }
            let (attacked, mobility_weight, pressure_weight) = match kind {
                PieceType::Knight => (attacks::knight_attacks(square), 5, 2),
                PieceType::Rook => {
                    let file = pawns::file_mask(square.file());
                    rook_file_score += if all_pawns & file == 0 {
                        27
                    } else if own_pawns & file == 0 {
                        15
                    } else {
                        0
                    };
                    (attacks::rook_attacks(square, all), 4, 3)
                }
                PieceType::Bishop => (attacks::bishop_attacks(square, all), 8, 2),
                PieceType::Queen => (attacks::queen_attacks(square, all), 3, 5),
                PieceType::King => continue,
                PieceType::Pawn => unreachable!(),
            };
            middlegame += (attacked & !own).count_ones() as i32 * mobility_weight;
            let hits = attacked & zone;
            if hits != 0 {
                attackers += 1;
                pressure += pressure_weight + hits.count_ones() as i32;
            }
        }
    }
    if bishop_count >= 2 {
        middlegame += 28;
        endgame += 41;
    }
    let (pawn_mg, pawn_eg) = pawns::evaluate(position, color);
    middlegame += pawn_mg + rook_file_score;
    endgame += pawn_eg;
    let king_pressure = if attackers < 2 && (attackers == 0 || !has_queen) {
        0
    } else {
        (pressure * (attackers + 1)).min(120)
    };
    ColorTerms {
        middlegame,
        endgame,
        phase,
        king_pressure,
    }
}

fn king_safety(position: &Position, color: Color, enemy_pressure: i32) -> i32 {
    let king = position.pieces(color, PieceType::King);
    if king == 0 {
        return 0;
    }
    let king = Square::from_index(king.trailing_zeros() as u8).expect("king bit is valid");
    let shield_rank = match color {
        Color::White => king.rank().checked_add(1),
        Color::Black => king.rank().checked_sub(1),
    };
    let mut shield = 0;
    if let Some(rank) = shield_rank {
        for delta in -1_i8..=1 {
            let file = king.file() as i8 + delta;
            if (0..8).contains(&file)
                && let Some(square) = Square::from_coords(file as u8, rank)
                && position.pieces(color, PieceType::Pawn) & square.bit() != 0
            {
                shield += 1;
            }
        }
    }
    shield * 14 - enemy_pressure
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coordinated_attack_scores_more_than_a_lone_minor() {
        let lone = Position::from_fen("k7/8/8/8/8/5n2/8/6K1 w - - 0 1").unwrap();
        let coordinated = Position::from_fen("k7/8/8/8/8/5n2/7r/6K1 w - - 0 1").unwrap();

        assert_eq!(color_terms(&lone, Color::Black).king_pressure, 0);
        assert!(color_terms(&coordinated, Color::Black).king_pressure > 0);
    }
}
