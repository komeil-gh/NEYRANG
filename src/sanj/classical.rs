use crate::chess::{Color, PieceType, Position, Square, attacks};

use super::{pawns, psqt};

pub const TEMPO: i32 = 6;

const MG_VALUE: [i32; 6] = [69, 300, 312, 405, 1_094, 0];
const EG_VALUE: [i32; 6] = [79, 280, 330, 589, 1_077, 0];
const PHASE_WEIGHT: [i32; 6] = [0, 1, 1, 2, 4, 0];
const MAX_PHASE: i32 = 24;
const PAWN_LEVERAGE: [i32; 6] = [0, 8, 9, 18, 28, 0];

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
            endgame += sign * 41;
        }
        let (pawn_mg, pawn_eg) = pawns::evaluate(position, color);
        middlegame += sign * pawn_mg;
        endgame += sign * pawn_eg;
        middlegame += sign * mobility(position, color);
        middlegame += sign * rook_files(position, color);
        middlegame += sign * king_safety(position, color);
        let leverage = latent_pawn_leverage(position, color);
        middlegame += sign * leverage;
        endgame += sign * leverage;
    }

    phase = phase.clamp(0, MAX_PHASE);
    let white_score = (middlegame * phase + endgame * (MAX_PHASE - phase)) / MAX_PHASE;
    match position.side_to_move() {
        Color::White => white_score + TEMPO,
        Color::Black => -white_score + TEMPO,
    }
}

fn latent_pawn_leverage(position: &Position, color: Color) -> i32 {
    let enemy = color.opposite();
    let enemy_non_pawns = position.occupancy(enemy)
        & !position.pieces(enemy, PieceType::Pawn)
        & !position.pieces(enemy, PieceType::King);
    let mut enemy_pawn_attacks = 0;
    let mut enemy_pawns = position.pieces(enemy, PieceType::Pawn);
    while enemy_pawns != 0 {
        let square = Square::from_index(enemy_pawns.trailing_zeros() as u8)
            .expect("piece bit is a valid square");
        enemy_pawns &= enemy_pawns - 1;
        enemy_pawn_attacks |= attacks::pawn_attacks(enemy, square);
    }

    let mut score = 0;
    let mut pawns = position.pieces(color, PieceType::Pawn);
    while pawns != 0 {
        let square =
            Square::from_index(pawns.trailing_zeros() as u8).expect("piece bit is valid square");
        pawns &= pawns - 1;
        score += pawn_leverage_value(
            position,
            enemy,
            attacks::pawn_attacks(color, square) & enemy_non_pawns,
        );

        let next_rank = match color {
            Color::White => square.rank().checked_add(1),
            Color::Black => square.rank().checked_sub(1),
        };
        let Some(destination) = next_rank.and_then(|rank| Square::from_coords(square.file(), rank))
        else {
            continue;
        };
        let promotes = match color {
            Color::White => destination.rank() == 7,
            Color::Black => destination.rank() == 0,
        };
        if promotes
            || position.all_occupancy() & destination.bit() != 0
            || enemy_pawn_attacks & destination.bit() != 0
        {
            continue;
        }
        score += pawn_leverage_value(
            position,
            enemy,
            attacks::pawn_attacks(color, destination) & enemy_non_pawns,
        ) / 2;
    }
    score.min(80)
}

fn pawn_leverage_value(position: &Position, enemy: Color, targets: u64) -> i32 {
    PieceType::ALL
        .iter()
        .map(|&kind| {
            (targets & position.pieces(enemy, kind)).count_ones() as i32
                * PAWN_LEVERAGE[kind.index()]
        })
        .sum()
}

fn mobility(position: &Position, color: Color) -> i32 {
    let own = position.occupancy(color);
    let all = position.all_occupancy();
    let mut score = 0;
    for (kind, weight) in [
        (PieceType::Knight, 5),
        (PieceType::Bishop, 8),
        (PieceType::Rook, 4),
        (PieceType::Queen, 3),
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
            score += 27;
        } else if own_pawns & file == 0 {
            score += 15;
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
    shield * 14 - king_pressure(position, color, king)
}

fn king_pressure(position: &Position, color: Color, king: Square) -> i32 {
    let enemy = color.opposite();
    let occupancy = position.all_occupancy();
    let zone = attacks::king_attacks(king) | king.bit();
    let mut pressure = 0;

    let mut pawns = position.pieces(enemy, PieceType::Pawn);
    while pawns != 0 {
        let index = pawns.trailing_zeros() as u8;
        pawns &= pawns - 1;
        let square = Square::from_index(index).expect("piece bit is a valid square");
        pressure += (attacks::pawn_attacks(enemy, square) & zone).count_ones() as i32;
    }

    let mut attackers = 0;
    for (kind, weight) in [
        (PieceType::Knight, 2),
        (PieceType::Bishop, 2),
        (PieceType::Rook, 3),
        (PieceType::Queen, 5),
    ] {
        let mut pieces = position.pieces(enemy, kind);
        while pieces != 0 {
            let index = pieces.trailing_zeros() as u8;
            pieces &= pieces - 1;
            let square = Square::from_index(index).expect("piece bit is a valid square");
            let hits = match kind {
                PieceType::Knight => attacks::knight_attacks(square),
                PieceType::Bishop => attacks::bishop_attacks(square, occupancy),
                PieceType::Rook => attacks::rook_attacks(square, occupancy),
                PieceType::Queen => attacks::queen_attacks(square, occupancy),
                _ => 0,
            } & zone;
            if hits != 0 {
                attackers += 1;
                pressure += weight + hits.count_ones() as i32;
            }
        }
    }

    if attackers < 2 && (attackers == 0 || position.pieces(enemy, PieceType::Queen) == 0) {
        return 0;
    }
    (pressure * (attackers + 1)).min(120)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coordinated_attack_scores_more_than_a_lone_minor() {
        let lone = Position::from_fen("k7/8/8/8/8/5n2/8/6K1 w - - 0 1").unwrap();
        let coordinated = Position::from_fen("k7/8/8/8/8/5n2/7r/6K1 w - - 0 1").unwrap();
        let king = Square::G1;

        assert_eq!(king_pressure(&lone, Color::White, king), 0);
        assert!(king_pressure(&coordinated, Color::White, king) > 0);
    }

    #[test]
    fn pawn_leverage_values_direct_and_safe_latent_threats() {
        let direct = Position::from_fen("6k1/8/3r1q2/4P3/8/8/8/6K1 w - - 0 1").unwrap();
        let latent = Position::from_fen("6k1/3r1q2/8/4P3/8/8/8/6K1 w - - 0 1").unwrap();
        let unsafe_push = Position::from_fen("6k1/3p1q2/8/4P3/8/8/8/6K1 w - - 0 1").unwrap();

        assert_eq!(latent_pawn_leverage(&direct, Color::White), 46);
        assert_eq!(latent_pawn_leverage(&latent, Color::White), 23);
        assert_eq!(latent_pawn_leverage(&unsafe_push, Color::White), 0);
    }
}
