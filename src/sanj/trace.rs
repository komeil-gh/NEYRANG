use std::fmt;

use crate::chess::{Color, PieceType, Position, Square, attacks};

pub const TRACE_SCHEMA: &str = "neyrang-sanj-trace-v2";

pub const TRACE_COLUMNS: &str = concat!(
    "stm\tphase\t",
    "piece_p_delta\tpiece_n_delta\tpiece_b_delta\tpiece_r_delta\t",
    "piece_q_delta\tpiece_k_delta\t",
    "pawn_rank_delta\tpawn_file_edge_delta\tknight_center_delta\t",
    "bishop_center_delta\trook_rank_delta\trook_file_edge_delta\t",
    "queen_center_delta\tking_rank_delta\tking_file_edge_delta\t",
    "king_center_delta\t",
    "bishop_pair_delta\tdoubled_extra_delta\tisolated_pawn_delta\t",
    "passed_rank_sq_delta\t",
    "mobility_n_delta\tmobility_b_delta\tmobility_r_delta\t",
    "mobility_q_delta\t",
    "rook_open_delta\trook_semi_open_delta\tking_shield_delta\tking_danger_delta\t",
    "middlegame_cp\tendgame_cp\twhite_cp\ttempo_cp\tstm_cp"
);

const MG_VALUE: [i32; 6] = [69, 300, 312, 405, 1_094, 0];
const EG_VALUE: [i32; 6] = [79, 280, 330, 589, 1_077, 0];
const PHASE_WEIGHT: [i32; 6] = [0, 1, 1, 2, 4, 0];
const MAX_PHASE: i32 = 24;
const TEMPO: i32 = 6;

/// Exact white-minus-black coefficients for the current classical evaluator.
///
/// This type is compiled only for tests or the explicit `sanj-tools` feature.
/// The tournament evaluator remains an independent, unchanged implementation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EvalTrace {
    pub side_to_move: Color,
    pub phase: i32,
    pub piece_count: [i32; 6],
    pub pawn_rank: i32,
    pub pawn_file_edge: i32,
    pub knight_center: i32,
    pub bishop_center: i32,
    pub rook_rank: i32,
    pub rook_file_edge: i32,
    pub queen_center: i32,
    pub king_rank: i32,
    pub king_file_edge: i32,
    pub king_center: i32,
    pub bishop_pair: i32,
    pub doubled_extra: i32,
    pub isolated_pawn: i32,
    pub passed_rank_sq: i32,
    pub mobility: [i32; 4],
    pub rook_open: i32,
    pub rook_semi_open: i32,
    pub king_shield: i32,
    pub king_danger: i32,
    pub middlegame: i32,
    pub endgame: i32,
    pub white_score: i32,
    pub final_score: i32,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct PawnCoefficients {
    doubled_extra: i32,
    isolated: i32,
    passed_rank_sq: i32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Totals {
    middlegame: i32,
    endgame: i32,
    white_score: i32,
    final_score: i32,
}

pub fn trace(position: &Position) -> EvalTrace {
    let mut result = EvalTrace {
        side_to_move: position.side_to_move(),
        phase: 0,
        piece_count: [0; 6],
        pawn_rank: 0,
        pawn_file_edge: 0,
        knight_center: 0,
        bishop_center: 0,
        rook_rank: 0,
        rook_file_edge: 0,
        queen_center: 0,
        king_rank: 0,
        king_file_edge: 0,
        king_center: 0,
        bishop_pair: 0,
        doubled_extra: 0,
        isolated_pawn: 0,
        passed_rank_sq: 0,
        mobility: [0; 4],
        rook_open: 0,
        rook_semi_open: 0,
        king_shield: 0,
        king_danger: 0,
        middlegame: 0,
        endgame: 0,
        white_score: 0,
        final_score: 0,
    };

    for color in [Color::White, Color::Black] {
        let sign = color_sign(color);
        for kind in PieceType::ALL {
            let mut pieces = position.pieces(color, kind);
            let count = pieces.count_ones() as i32;
            result.piece_count[kind.index()] += sign * count;
            result.phase += count * PHASE_WEIGHT[kind.index()];

            while pieces != 0 {
                let index = pieces.trailing_zeros() as u8;
                pieces &= pieces - 1;
                let square = Square::from_index(index).expect("piece bit is a valid square");
                add_psqt_bases(&mut result, kind, square, color, sign);
            }
        }

        if position.pieces(color, PieceType::Bishop).count_ones() >= 2 {
            result.bishop_pair += sign;
        }

        let pawns = pawn_coefficients(position, color);
        result.doubled_extra += sign * pawns.doubled_extra;
        result.isolated_pawn += sign * pawns.isolated;
        result.passed_rank_sq += sign * pawns.passed_rank_sq;

        let mobility = mobility_coefficients(position, color);
        for (target, coefficient) in result.mobility.iter_mut().zip(mobility) {
            *target += sign * coefficient;
        }

        let (open, semi_open) = rook_file_coefficients(position, color);
        result.rook_open += sign * open;
        result.rook_semi_open += sign * semi_open;
        result.king_shield += sign * king_shield_coefficient(position, color);
        result.king_danger += sign * king_danger_coefficient(position, color);
    }

    result.phase = result.phase.clamp(0, MAX_PHASE);
    let totals = result.recompute();
    result.middlegame = totals.middlegame;
    result.endgame = totals.endgame;
    result.white_score = totals.white_score;
    result.final_score = totals.final_score;
    result
}

impl EvalTrace {
    fn recompute(&self) -> Totals {
        let material_mg = dot(self.piece_count, MG_VALUE);
        let material_eg = dot(self.piece_count, EG_VALUE);

        let psqt_mg = self.pawn_rank * 3 + self.knight_center * 14
            - self.piece_count[PieceType::Knight.index()] * 24
            + self.bishop_center * 6
            - self.piece_count[PieceType::Bishop.index()] * 12
            + self.rook_rank * 4
            + self.rook_file_edge * 3
            + self.queen_center * 4
            - self.piece_count[PieceType::Queen.index()] * 5
            - self.king_rank * 6
            - self.king_file_edge * 5;
        let psqt_eg = self.pawn_rank * 6 + self.knight_center * 9
            - self.piece_count[PieceType::Knight.index()] * 18
            + self.bishop_center * 4
            - self.piece_count[PieceType::Bishop.index()] * 10
            + self.rook_rank * 4
            + self.queen_center
            + self.king_center * 4
            - self.piece_count[PieceType::King.index()] * 20;

        let middlegame = material_mg + psqt_mg + self.bishop_pair * 28
            - self.doubled_extra * 9
            - self.isolated_pawn * 10
            + self.mobility[0] * 5
            + self.mobility[1] * 8
            + self.mobility[2] * 4
            + self.mobility[3] * 3
            + self.rook_open * 27
            + self.rook_semi_open * 15
            + self.king_shield * 14
            - self.king_danger;
        let endgame = material_eg + psqt_eg + self.bishop_pair * 41
            - self.doubled_extra * 14
            - self.isolated_pawn * 9
            + self.passed_rank_sq * 6;
        let white_score =
            (middlegame * self.phase + endgame * (MAX_PHASE - self.phase)) / MAX_PHASE;
        let final_score = match self.side_to_move {
            Color::White => white_score + TEMPO,
            Color::Black => -white_score + TEMPO,
        };
        Totals {
            middlegame,
            endgame,
            white_score,
            final_score,
        }
    }
}

impl fmt::Display for EvalTrace {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let stm = match self.side_to_move {
            Color::White => 'w',
            Color::Black => 'b',
        };
        write!(
            formatter,
            concat!(
                "{}\t{}\t{}\t{}\t{}\t{}\t",
                "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t",
                "{}\t{}\t{}\t{}\t",
                "{}\t{}\t{}\t{}\t",
                "{}\t{}\t{}\t{}\t",
                "{}\t{}\t",
                "{}\t{}\t{}\t{}\t{}"
            ),
            stm,
            self.phase,
            self.piece_count[0],
            self.piece_count[1],
            self.piece_count[2],
            self.piece_count[3],
            self.piece_count[4],
            self.piece_count[5],
            self.pawn_rank,
            self.pawn_file_edge,
            self.knight_center,
            self.bishop_center,
            self.rook_rank,
            self.rook_file_edge,
            self.queen_center,
            self.king_rank,
            self.king_file_edge,
            self.king_center,
            self.bishop_pair,
            self.doubled_extra,
            self.isolated_pawn,
            self.passed_rank_sq,
            self.mobility[0],
            self.mobility[1],
            self.mobility[2],
            self.mobility[3],
            self.rook_open,
            self.rook_semi_open,
            self.king_shield,
            self.king_danger,
            self.middlegame,
            self.endgame,
            self.white_score,
            TEMPO,
            self.final_score,
        )
    }
}

fn add_psqt_bases(trace: &mut EvalTrace, kind: PieceType, square: Square, color: Color, sign: i32) {
    let rank = relative_rank(square, color);
    let file_edge = edge_distance(square.file() as i32);
    let rank_edge = edge_distance(rank);
    let center = file_edge + rank_edge;
    match kind {
        PieceType::Pawn => {
            trace.pawn_rank += sign * rank;
            trace.pawn_file_edge += sign * file_edge;
        }
        PieceType::Knight => trace.knight_center += sign * center,
        PieceType::Bishop => trace.bishop_center += sign * center,
        PieceType::Rook => {
            trace.rook_rank += sign * rank;
            trace.rook_file_edge += sign * file_edge;
        }
        PieceType::Queen => trace.queen_center += sign * center,
        PieceType::King => {
            trace.king_rank += sign * rank;
            trace.king_file_edge += sign * file_edge;
            trace.king_center += sign * center;
        }
    }
}

fn pawn_coefficients(position: &Position, color: Color) -> PawnCoefficients {
    let pawns = position.pieces(color, PieceType::Pawn);
    let enemy_pawns = position.pieces(color.opposite(), PieceType::Pawn);
    let mut result = PawnCoefficients::default();

    for file in 0..8 {
        let count = (pawns & file_mask(file)).count_ones() as i32;
        result.doubled_extra += (count - 1).max(0);
    }

    let mut remaining = pawns;
    while remaining != 0 {
        let index = remaining.trailing_zeros() as u8;
        remaining &= remaining - 1;
        let square = Square::from_index(index).expect("pawn bit is a valid square");
        if pawns & adjacent_file_mask(square.file()) == 0 {
            result.isolated += 1;
        }
        if enemy_pawns & passed_pawn_mask(square, color) == 0 {
            let advancement = relative_rank(square, color);
            result.passed_rank_sq += advancement * advancement;
        }
    }
    result
}

fn mobility_coefficients(position: &Position, color: Color) -> [i32; 4] {
    let own = position.occupancy(color);
    let all = position.all_occupancy();
    let mut result = [0; 4];
    for (index, kind) in [
        PieceType::Knight,
        PieceType::Bishop,
        PieceType::Rook,
        PieceType::Queen,
    ]
    .into_iter()
    .enumerate()
    {
        let mut pieces = position.pieces(color, kind);
        while pieces != 0 {
            let square_index = pieces.trailing_zeros() as u8;
            pieces &= pieces - 1;
            let square = Square::from_index(square_index).expect("piece bit is a valid square");
            let targets = match kind {
                PieceType::Knight => attacks::knight_attacks(square),
                PieceType::Bishop => attacks::bishop_attacks(square, all),
                PieceType::Rook => attacks::rook_attacks(square, all),
                PieceType::Queen => attacks::queen_attacks(square, all),
                _ => 0,
            };
            result[index] += (targets & !own).count_ones() as i32;
        }
    }
    result
}

fn rook_file_coefficients(position: &Position, color: Color) -> (i32, i32) {
    let own_pawns = position.pieces(color, PieceType::Pawn);
    let all_pawns = own_pawns | position.pieces(color.opposite(), PieceType::Pawn);
    let mut rooks = position.pieces(color, PieceType::Rook);
    let mut open = 0;
    let mut semi_open = 0;
    while rooks != 0 {
        let index = rooks.trailing_zeros() as u8;
        rooks &= rooks - 1;
        let square = Square::from_index(index).expect("rook bit is a valid square");
        let file = file_mask(square.file());
        if all_pawns & file == 0 {
            open += 1;
        } else if own_pawns & file == 0 {
            semi_open += 1;
        }
    }
    (open, semi_open)
}

fn king_shield_coefficient(position: &Position, color: Color) -> i32 {
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
    shield
}

fn king_danger_coefficient(position: &Position, color: Color) -> i32 {
    let king = position.pieces(color, PieceType::King);
    if king == 0 {
        return 0;
    }
    let king = Square::from_index(king.trailing_zeros() as u8).expect("king bit is valid");
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

const fn color_sign(color: Color) -> i32 {
    match color {
        Color::White => 1,
        Color::Black => -1,
    }
}

const fn relative_rank(square: Square, color: Color) -> i32 {
    match color {
        Color::White => square.rank() as i32,
        Color::Black => 7 - square.rank() as i32,
    }
}

const fn edge_distance(coordinate: i32) -> i32 {
    let low = coordinate;
    let high = 7 - coordinate;
    if low < high { low } else { high }
}

const fn file_mask(file: u8) -> u64 {
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

fn passed_pawn_mask(square: Square, color: Color) -> u64 {
    let start = match color {
        Color::White => square.rank() + 1,
        Color::Black => 0,
    };
    let end = match color {
        Color::White => 8,
        Color::Black => square.rank(),
    };
    let mut mask = 0_u64;
    for rank in start..end {
        for delta in -1_i8..=1 {
            let file = square.file() as i8 + delta;
            if (0..8).contains(&file)
                && let Some(target) = Square::from_coords(file as u8, rank)
            {
                mask |= target.bit();
            }
        }
    }
    mask
}

fn dot(left: [i32; 6], right: [i32; 6]) -> i32 {
    left.into_iter().zip(right).map(|(a, b)| a * b).sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sanj::evaluate;

    #[derive(Default)]
    struct Coverage {
        white_to_move: bool,
        black_to_move: bool,
        bishop_pair: bool,
        doubled: bool,
        isolated: bool,
        passed: bool,
        mobility: bool,
        rook_file: bool,
        king_shield: bool,
        king_danger: bool,
    }

    #[test]
    fn trace_header_and_row_have_stable_width() {
        let row = trace(&Position::startpos()).to_string();
        assert_eq!(TRACE_SCHEMA, "neyrang-sanj-trace-v2");
        assert_eq!(TRACE_COLUMNS.split('\t').count(), row.split('\t').count());
        assert_eq!(row.split('\t').count(), 35);
    }

    #[test]
    fn symmetric_start_position_contains_only_tempo() {
        let result = trace(&Position::startpos());
        assert_eq!(result.phase, MAX_PHASE);
        assert_eq!(result.piece_count, [0; 6]);
        assert_eq!(result.middlegame, 0);
        assert_eq!(result.endgame, 0);
        assert_eq!(result.white_score, 0);
        assert_eq!(result.final_score, TEMPO);
    }

    #[test]
    fn trace_reconstructs_production_on_100_000_legal_positions() {
        let mut random = SplitMix64(0xE7A1_7ACE_2026_0830);
        let mut position = Position::startpos();
        let mut game_ply = 0;
        let mut coverage = Coverage::default();

        for index in 0..100_000 {
            let result = trace(&position);
            let recomputed = result.recompute();
            assert_eq!(result.middlegame, recomputed.middlegame);
            assert_eq!(result.endgame, recomputed.endgame);
            assert_eq!(result.white_score, recomputed.white_score);
            assert_eq!(result.final_score, recomputed.final_score);
            assert_eq!(
                result.final_score,
                evaluate(&position),
                "trace mismatch at corpus position {index}: {}",
                position.to_fen()
            );
            coverage.observe(result);

            let legal = position.legal_moves();
            if legal.is_empty() || game_ply == 191 {
                position = Position::startpos();
                game_ply = 0;
                continue;
            }
            let mv = legal.as_slice()[(random.next() as usize) % legal.len()];
            position.make_move(mv);
            game_ply += 1;
        }

        assert!(coverage.white_to_move && coverage.black_to_move);
        assert!(coverage.bishop_pair);
        assert!(coverage.doubled);
        assert!(coverage.isolated);
        assert!(coverage.passed);
        assert!(coverage.mobility);
        assert!(coverage.rook_file);
        assert!(coverage.king_shield);
        assert!(coverage.king_danger);
    }

    impl Coverage {
        fn observe(&mut self, trace: EvalTrace) {
            self.white_to_move |= trace.side_to_move == Color::White;
            self.black_to_move |= trace.side_to_move == Color::Black;
            self.bishop_pair |= trace.bishop_pair != 0;
            self.doubled |= trace.doubled_extra != 0;
            self.isolated |= trace.isolated_pawn != 0;
            self.passed |= trace.passed_rank_sq != 0;
            self.mobility |= trace.mobility.iter().any(|value| *value != 0);
            self.rook_file |= trace.rook_open != 0 || trace.rook_semi_open != 0;
            self.king_shield |= trace.king_shield != 0;
            self.king_danger |= trace.king_danger != 0;
        }
    }

    struct SplitMix64(u64);

    impl SplitMix64 {
        fn next(&mut self) -> u64 {
            self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
            let mut value = self.0;
            value = (value ^ (value >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            value = (value ^ (value >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
            value ^ (value >> 31)
        }
    }
}
