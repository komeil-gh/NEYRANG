use crate::chess::{Color, PieceType, Position, Square, attacks};

use super::{pawns, psqt};

pub const TEMPO: i32 = 6;

const MG_VALUE: [i32; 6] = [69, 300, 312, 405, 1_094, 0];
const EG_VALUE: [i32; 6] = [79, 280, 330, 589, 1_077, 0];
const PHASE_WEIGHT: [i32; 6] = [0, 1, 1, 2, 4, 0];
const MAX_PHASE: i32 = 24;

/// Return centipawns from the side-to-move perspective.
pub fn evaluate(position: &Position) -> i32 {
    let mut middlegame = 0_i32;
    let mut endgame = 0_i32;
    let mut phase = 0_i32;
    let activity = [
        piece_activity(position, Color::White),
        piece_activity(position, Color::Black),
    ];

    for color in [Color::White, Color::Black] {
        let sign = if color == Color::White { 1 } else { -1 };
        for kind in PieceType::ALL {
            let mut pieces = position.pieces(color, kind);
            phase += pieces.count_ones() as i32 * PHASE_WEIGHT[kind.index()];
            while pieces != 0 {
                let index = pieces.trailing_zeros() as u8;
                pieces &= pieces - 1;
                let square = Square::from_index(index).expect("piece bit is a valid square");
                middlegame +=
                    sign * (MG_VALUE[kind.index()] + psqt::middlegame(kind, square, color));
                endgame += sign * (EG_VALUE[kind.index()] + psqt::endgame(kind, square, color));
            }
        }

        if position.pieces(color, PieceType::Bishop).count_ones() >= 2 {
            middlegame += sign * 28;
            endgame += sign * 41;
        }
        let (pawn_mg, pawn_eg) = pawns::evaluate(position, color);
        middlegame += sign * pawn_mg;
        endgame += sign * pawn_eg;
        middlegame += sign * activity[color.index()].mobility;
        middlegame += sign * rook_files(position, color);
        middlegame += sign
            * king_safety(
                position,
                color,
                activity[color.opposite().index()].king_pressure,
            );
    }

    phase = phase.clamp(0, MAX_PHASE);
    let white_score = (middlegame * phase + endgame * (MAX_PHASE - phase)) / MAX_PHASE;
    match position.side_to_move() {
        Color::White => white_score + TEMPO,
        Color::Black => -white_score + TEMPO,
    }
}

#[derive(Clone, Copy)]
struct Activity {
    mobility: i32,
    king_pressure: i32,
}

fn piece_activity(position: &Position, color: Color) -> Activity {
    let own = position.occupancy(color);
    let all = position.all_occupancy();
    let enemy_king = position.pieces(color.opposite(), PieceType::King);
    let zone = if enemy_king == 0 {
        0
    } else {
        let square =
            Square::from_index(enemy_king.trailing_zeros() as u8).expect("king bit is valid");
        attacks::king_attacks(square) | square.bit()
    };
    let mut mobility = 0;
    let mut pressure = 0;
    let mut pawns = position.pieces(color, PieceType::Pawn);
    while pawns != 0 {
        let index = pawns.trailing_zeros() as u8;
        pawns &= pawns - 1;
        let square = Square::from_index(index).expect("piece bit is a valid square");
        pressure += (attacks::pawn_attacks(color, square) & zone).count_ones() as i32;
    }

    let mut attackers = 0;
    for (kind, weight) in [
        (PieceType::Knight, (5, 2)),
        (PieceType::Bishop, (8, 2)),
        (PieceType::Rook, (4, 3)),
        (PieceType::Queen, (3, 5)),
    ] {
        let mut pieces = position.pieces(color, kind);
        while pieces != 0 {
            let index = pieces.trailing_zeros() as u8;
            pieces &= pieces - 1;
            let square = Square::from_index(index).expect("piece bit is a valid square");
            let attacks = match kind {
                PieceType::Knight => attacks::knight_attacks(square),
                PieceType::Bishop => attacks::bishop_attacks(square, all),
                PieceType::Rook => attacks::rook_attacks(square, all),
                PieceType::Queen => attacks::queen_attacks(square, all),
                _ => 0,
            };
            mobility += (attacks & !own).count_ones() as i32 * weight.0;
            let hits = attacks & zone;
            if hits != 0 {
                attackers += 1;
                pressure += weight.1 + hits.count_ones() as i32;
            }
        }
    }
    let king_pressure =
        if attackers < 2 && (attackers == 0 || position.pieces(color, PieceType::Queen) == 0) {
            0
        } else {
            (pressure * (attackers + 1)).min(120)
        };
    Activity {
        mobility,
        king_pressure,
    }
}

fn rook_files(position: &Position, color: Color) -> i32 {
    let own_pawns = position.pieces(color, PieceType::Pawn);
    let all_pawns = own_pawns | position.pieces(color.opposite(), PieceType::Pawn);
    let mut rooks = position.pieces(color, PieceType::Rook);
    let mut score = 0;
    while rooks != 0 {
        let index = rooks.trailing_zeros() as u8;
        rooks &= rooks - 1;
        let square = Square::from_index(index).expect("rook bit is a valid square");
        let file = pawns::file_mask(square.file());
        if all_pawns & file == 0 {
            score += 27;
        } else if own_pawns & file == 0 {
            score += 15;
        }
    }
    score
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

        assert_eq!(piece_activity(&lone, Color::Black).king_pressure, 0);
        assert!(piece_activity(&coordinated, Color::Black).king_pressure > 0);
    }
}
