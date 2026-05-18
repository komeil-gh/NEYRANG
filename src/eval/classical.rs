use crate::chess::{Color, PieceType, Position, Square, attacks};

use super::{pawns, psqt};

pub const TEMPO: i32 = 12;

const MG_VALUE: [i32; 6] = [82, 337, 365, 477, 1_025, 0];
const EG_VALUE: [i32; 6] = [94, 281, 297, 512, 936, 0];
const PHASE_WEIGHT: [i32; 6] = [0, 1, 1, 2, 4, 0];
const MAX_PHASE: i32 = 24;

/// Return centipawns from the side-to-move perspective.
pub fn evaluate(position: &Position) -> i32 {
    let mut middlegame = 0_i32;
    let mut endgame = 0_i32;
    let mut phase = 0_i32;

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
            endgame += sign * 38;
        }
        let (pawn_mg, pawn_eg) = pawns::evaluate(position, color);
        middlegame += sign * pawn_mg;
        endgame += sign * pawn_eg;
        middlegame += sign * mobility(position, color);
        middlegame += sign * rook_files(position, color);
        middlegame += sign * king_safety(position, color);
    }

    phase = phase.clamp(0, MAX_PHASE);
    let white_score = (middlegame * phase + endgame * (MAX_PHASE - phase)) / MAX_PHASE;
    match position.side_to_move() {
        Color::White => white_score + TEMPO,
        Color::Black => -white_score + TEMPO,
    }
}

fn mobility(position: &Position, color: Color) -> i32 {
    let own = position.occupancy(color);
    let all = position.all_occupancy();
    let mut score = 0;
    for (kind, weight) in [
        (PieceType::Knight, 4),
        (PieceType::Bishop, 5),
        (PieceType::Rook, 2),
        (PieceType::Queen, 1),
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
            score += (attacks & !own).count_ones() as i32 * weight;
        }
    }
    score
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
            score += 18;
        } else if own_pawns & file == 0 {
            score += 10;
        }
    }
    score
}

fn king_safety(position: &Position, color: Color) -> i32 {
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
    shield * 9
}
