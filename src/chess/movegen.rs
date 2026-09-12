use super::{
    Bitboard, CastlingRights, Color, Move, MoveFlag, MoveList, PieceType, Position, Square, attacks,
};

pub(crate) fn generate_legal(position: &mut Position) -> MoveList {
    let pseudo = generate_pseudo_legal(position);
    let moving_color = position.side_to_move();
    let Some(king_square) = single_square(position.pieces(moving_color, PieceType::King)) else {
        return MoveList::new();
    };
    let enemy = moving_color.opposite();
    let checkers = attackers_to(position, king_square, enemy, position.all_occupancy(), 0);
    let pinned = if checkers == 0 {
        absolute_pins(position, moving_color, king_square)
    } else {
        0
    };
    let mut legal = MoveList::new();
    for &mv in pseudo.iter() {
        let requires_validation = checkers != 0
            || mv.from() == king_square
            || mv.is_en_passant()
            || pinned & mv.from().bit() != 0;
        if !requires_validation || king_is_safe_after_move(position, mv, moving_color, king_square)
        {
            legal.push(mv);
        }
    }
    legal
}

pub(crate) fn has_legal_move(position: &mut Position) -> bool {
    let color = position.side_to_move();
    let Some(king) = single_square(position.pieces(color, PieceType::King)) else {
        return false;
    };
    // Prove a single push legal before materializing all pawn moves.
    let pawns = position.pieces(color, PieceType::Pawn);
    let mut pushes = match color {
        Color::White => pawns << 8,
        Color::Black => pawns >> 8,
    } & !position.all_occupancy();
    while let Some(to) = pop_square(&mut pushes) {
        let from_rank = match color {
            Color::White => to.rank() - 1,
            Color::Black => to.rank() + 1,
        };
        let from = Square::from_coords(to.file(), from_rank)
            .expect("a shifted pawn has an on-board origin");
        let flag = if to.rank() == promotion_rank(color) {
            MoveFlag::QueenPromotion
        } else {
            MoveFlag::Quiet
        };
        if king_is_safe_after_move(position, Move::new(from, to, flag), color, king) {
            return true;
        }
    }
    // A legal pawn move proves non-terminal status without generating every piece.
    let mut pawns = MoveList::new();
    generate_pawns(position, &mut pawns);
    if pawns
        .iter()
        .any(|&mv| king_is_safe_after_move(position, mv, color, king))
    {
        return true;
    }
    !generate_legal(position).is_empty()
}

#[cfg(test)]
fn generate_legal_reference(position: &mut Position) -> MoveList {
    let pseudo = generate_pseudo_legal(position);
    let moving_color = position.side_to_move();
    let mut legal = MoveList::new();
    for &mv in pseudo.iter() {
        let undo = position.make_move(mv);
        if !is_in_check(position, moving_color) {
            legal.push(mv);
        }
        position.unmake_move(mv, undo);
    }
    legal
}

fn generate_pseudo_legal(position: &Position) -> MoveList {
    let mut moves = MoveList::new();
    generate_pawns(position, &mut moves);
    generate_leapers(position, PieceType::Knight, &mut moves);
    generate_sliders(position, PieceType::Bishop, &mut moves);
    generate_sliders(position, PieceType::Rook, &mut moves);
    generate_sliders(position, PieceType::Queen, &mut moves);
    generate_king(position, &mut moves);
    moves
}

pub(crate) fn is_in_check(position: &Position, color: Color) -> bool {
    let king = position.pieces(color, PieceType::King);
    if king.count_ones() != 1 {
        return true;
    }
    let square = Square::from_index(king.trailing_zeros() as u8)
        .expect("a king bit must identify a valid square");
    is_square_attacked(position, square, color.opposite())
}

pub(crate) fn is_square_attacked(position: &Position, square: Square, by: Color) -> bool {
    attackers_to(position, square, by, position.all_occupancy(), 0) != 0
}

fn king_is_safe_after_move(
    position: &Position,
    mv: Move,
    moving_color: Color,
    king_square: Square,
) -> bool {
    let from = mv.from();
    let to = mv.to();
    let capture_square = if mv.is_en_passant() {
        Square::from_coords(to.file(), from.rank())
            .expect("an en-passant capture square is on the board")
    } else {
        to
    };
    let removed_enemy = if mv.is_capture() {
        capture_square.bit()
    } else {
        0
    };

    let mut occupancy = position.all_occupancy() & !from.bit() & !removed_enemy;
    occupancy |= to.bit();
    if mv.is_castle() {
        let (rook_from, rook_to) = castle_rook_squares(mv);
        occupancy = occupancy & !rook_from.bit() | rook_to.bit();
    }

    let final_king_square = if from == king_square { to } else { king_square };
    attackers_to(
        position,
        final_king_square,
        moving_color.opposite(),
        occupancy,
        removed_enemy,
    ) == 0
}

fn attackers_to(
    position: &Position,
    square: Square,
    by: Color,
    occupancy: Bitboard,
    removed: Bitboard,
) -> Bitboard {
    let pawns = position.pieces(by, PieceType::Pawn) & !removed;
    let knights = position.pieces(by, PieceType::Knight) & !removed;
    let kings = position.pieces(by, PieceType::King) & !removed;
    let diagonal =
        (position.pieces(by, PieceType::Bishop) | position.pieces(by, PieceType::Queen)) & !removed;
    let orthogonal =
        (position.pieces(by, PieceType::Rook) | position.pieces(by, PieceType::Queen)) & !removed;

    (attacks::pawn_attacks(by.opposite(), square) & pawns)
        | (attacks::knight_attacks(square) & knights)
        | (attacks::king_attacks(square) & kings)
        | (attacks::bishop_attacks(square, occupancy) & diagonal)
        | (attacks::rook_attacks(square, occupancy) & orthogonal)
}

fn absolute_pins(position: &Position, color: Color, king_square: Square) -> Bitboard {
    const DIRECTIONS: [(i8, i8, bool); 8] = [
        (-1, -1, true),
        (-1, 0, false),
        (-1, 1, true),
        (0, -1, false),
        (0, 1, false),
        (1, -1, true),
        (1, 0, false),
        (1, 1, true),
    ];

    let own = position.occupancy(color);
    let enemy = color.opposite();
    let diagonal_sliders =
        position.pieces(enemy, PieceType::Bishop) | position.pieces(enemy, PieceType::Queen);
    let orthogonal_sliders =
        position.pieces(enemy, PieceType::Rook) | position.pieces(enemy, PieceType::Queen);
    let mut pinned = 0;

    for (file_step, rank_step, diagonal) in DIRECTIONS {
        let mut file = king_square.file() as i8 + file_step;
        let mut rank = king_square.rank() as i8 + rank_step;
        let mut candidate = 0;
        while (0..8).contains(&file) && (0..8).contains(&rank) {
            let square = Square::from_coords(file as u8, rank as u8)
                .expect("validated pin-ray coordinates are on the board");
            let bit = square.bit();
            if position.all_occupancy() & bit != 0 {
                if candidate == 0 {
                    if own & bit == 0 {
                        break;
                    }
                    candidate = bit;
                } else {
                    let compatible_sliders = if diagonal {
                        diagonal_sliders
                    } else {
                        orthogonal_sliders
                    };
                    if compatible_sliders & bit != 0 {
                        pinned |= candidate;
                    }
                    break;
                }
            }
            file += file_step;
            rank += rank_step;
        }
    }
    pinned
}

fn castle_rook_squares(mv: Move) -> (Square, Square) {
    let rank = mv.from().rank();
    let (from_file, to_file) = match mv.flag() {
        MoveFlag::KingCastle => (7, 5),
        MoveFlag::QueenCastle => (0, 3),
        _ => unreachable!("only castling moves relocate a rook"),
    };
    (
        Square::from_coords(from_file, rank).expect("a castling rook starts on the board"),
        Square::from_coords(to_file, rank).expect("a castling rook ends on the board"),
    )
}

fn generate_pawns(position: &Position, moves: &mut MoveList) {
    let color = position.side_to_move();
    let enemy = position.occupancy(color.opposite());
    let mut pawns = position.pieces(color, PieceType::Pawn);
    while let Some(from) = pop_square(&mut pawns) {
        let one_rank = match color {
            Color::White => from.rank().checked_add(1),
            Color::Black => from.rank().checked_sub(1),
        };
        if let Some(rank) = one_rank
            && let Some(to) = Square::from_coords(from.file(), rank)
            && position.all_occupancy() & to.bit() == 0
        {
            if rank == promotion_rank(color) {
                add_promotions(moves, from, to, false);
            } else {
                moves.push(Move::new(from, to, MoveFlag::Quiet));
                if from.rank() == pawn_start_rank(color) {
                    let double_rank = match color {
                        Color::White => from.rank() + 2,
                        Color::Black => from.rank() - 2,
                    };
                    let double_to = Square::from_coords(from.file(), double_rank)
                        .expect("double pawn destination is on the board");
                    if position.all_occupancy() & double_to.bit() == 0 {
                        moves.push(Move::new(from, double_to, MoveFlag::DoublePawnPush));
                    }
                }
            }
        }

        let mut captures = attacks::pawn_attacks(color, from) & enemy;
        while let Some(to) = pop_square(&mut captures) {
            if to.rank() == promotion_rank(color) {
                add_promotions(moves, from, to, true);
            } else {
                moves.push(Move::new(from, to, MoveFlag::Capture));
            }
        }
        if let Some(target) = position.en_passant()
            && attacks::pawn_attacks(color, from) & target.bit() != 0
        {
            moves.push(Move::new(from, target, MoveFlag::EnPassant));
        }
    }
}

fn generate_leapers(position: &Position, kind: PieceType, moves: &mut MoveList) {
    let color = position.side_to_move();
    let own = position.occupancy(color);
    let enemy = position.occupancy(color.opposite());
    let mut pieces = position.pieces(color, kind);
    while let Some(from) = pop_square(&mut pieces) {
        let mut targets = attacks::knight_attacks(from) & !own;
        while let Some(to) = pop_square(&mut targets) {
            moves.push(Move::new(
                from,
                to,
                if enemy & to.bit() != 0 {
                    MoveFlag::Capture
                } else {
                    MoveFlag::Quiet
                },
            ));
        }
    }
}

fn generate_sliders(position: &Position, kind: PieceType, moves: &mut MoveList) {
    let color = position.side_to_move();
    let own = position.occupancy(color);
    let enemy = position.occupancy(color.opposite());
    let mut pieces = position.pieces(color, kind);
    while let Some(from) = pop_square(&mut pieces) {
        let attacks = match kind {
            PieceType::Bishop => attacks::bishop_attacks(from, position.all_occupancy()),
            PieceType::Rook => attacks::rook_attacks(from, position.all_occupancy()),
            PieceType::Queen => attacks::queen_attacks(from, position.all_occupancy()),
            _ => unreachable!("only sliders reach slider generation"),
        };
        let mut targets = attacks & !own;
        while let Some(to) = pop_square(&mut targets) {
            moves.push(Move::new(
                from,
                to,
                if enemy & to.bit() != 0 {
                    MoveFlag::Capture
                } else {
                    MoveFlag::Quiet
                },
            ));
        }
    }
}

fn generate_king(position: &Position, moves: &mut MoveList) {
    let color = position.side_to_move();
    let own = position.occupancy(color);
    let enemy = position.occupancy(color.opposite());
    let king = position.pieces(color, PieceType::King);
    let Some(from) = single_square(king) else {
        return;
    };
    let mut targets = attacks::king_attacks(from) & !own;
    while let Some(to) = pop_square(&mut targets) {
        moves.push(Move::new(
            from,
            to,
            if enemy & to.bit() != 0 {
                MoveFlag::Capture
            } else {
                MoveFlag::Quiet
            },
        ));
    }
    generate_castling(position, color, moves);
}

fn generate_castling(position: &Position, color: Color, moves: &mut MoveList) {
    let enemy = color.opposite();
    let (king_from, king_to, through, rook_square, empty, right) = match color {
        Color::White => (
            Square::E1,
            Square::G1,
            Square::F1,
            Square::H1,
            Square::F1.bit() | Square::G1.bit(),
            CastlingRights::WHITE_KING,
        ),
        Color::Black => (
            Square::E8,
            Square::G8,
            Square::F8,
            Square::H8,
            Square::F8.bit() | Square::G8.bit(),
            CastlingRights::BLACK_KING,
        ),
    };
    if position.castling_rights().contains(right)
        && position.piece_at(king_from) == Some((color, PieceType::King))
        && position.piece_at(rook_square) == Some((color, PieceType::Rook))
        && position.all_occupancy() & empty == 0
        && !is_square_attacked(position, king_from, enemy)
        && !is_square_attacked(position, through, enemy)
        && !is_square_attacked(position, king_to, enemy)
    {
        moves.push(Move::new(king_from, king_to, MoveFlag::KingCastle));
    }

    let (king_to, through, rook_square, empty, right) = match color {
        Color::White => (
            Square::C1,
            Square::D1,
            Square::A1,
            Square::B1.bit() | Square::C1.bit() | Square::D1.bit(),
            CastlingRights::WHITE_QUEEN,
        ),
        Color::Black => (
            Square::C8,
            Square::D8,
            Square::A8,
            Square::B8.bit() | Square::C8.bit() | Square::D8.bit(),
            CastlingRights::BLACK_QUEEN,
        ),
    };
    if position.castling_rights().contains(right)
        && position.piece_at(king_from) == Some((color, PieceType::King))
        && position.piece_at(rook_square) == Some((color, PieceType::Rook))
        && position.all_occupancy() & empty == 0
        && !is_square_attacked(position, king_from, enemy)
        && !is_square_attacked(position, through, enemy)
        && !is_square_attacked(position, king_to, enemy)
    {
        moves.push(Move::new(king_from, king_to, MoveFlag::QueenCastle));
    }
}

fn add_promotions(moves: &mut MoveList, from: Square, to: Square, capture: bool) {
    let flags = if capture {
        [
            MoveFlag::KnightPromotionCapture,
            MoveFlag::BishopPromotionCapture,
            MoveFlag::RookPromotionCapture,
            MoveFlag::QueenPromotionCapture,
        ]
    } else {
        [
            MoveFlag::KnightPromotion,
            MoveFlag::BishopPromotion,
            MoveFlag::RookPromotion,
            MoveFlag::QueenPromotion,
        ]
    };
    for flag in flags {
        moves.push(Move::new(from, to, flag));
    }
}

const fn pawn_start_rank(color: Color) -> u8 {
    match color {
        Color::White => 1,
        Color::Black => 6,
    }
}

const fn promotion_rank(color: Color) -> u8 {
    match color {
        Color::White => 7,
        Color::Black => 0,
    }
}

fn single_square(bitboard: Bitboard) -> Option<Square> {
    if bitboard.count_ones() != 1 {
        return None;
    }
    Square::from_index(bitboard.trailing_zeros() as u8)
}

fn pop_square(bitboard: &mut Bitboard) -> Option<Square> {
    if *bitboard == 0 {
        return None;
    }
    let index = bitboard.trailing_zeros() as u8;
    *bitboard &= *bitboard - 1;
    Square::from_index(index)
}

#[cfg(test)]
mod tests {
    use super::*;

    const CURATED_FENS: [&str; 12] = [
        Position::STARTPOS_FEN,
        "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1",
        "8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1",
        "7k/8/8/8/3pP3/8/8/K7 b - e3 0 1",
        "4r2k/8/8/3pP3/8/8/8/4K3 w - d6 0 1",
        "4r2k/8/8/8/1b6/8/8/4K3 w - - 0 1",
        "4r2k/8/8/8/8/8/4R3/4K3 w - - 0 1",
        "7k/P7/8/8/8/8/7p/K7 w - - 0 1",
        "7k/P7/8/8/8/8/7p/K7 b - - 0 1",
        "7k/5Q2/6K1/8/8/8/8/8 b - - 0 1",
        "7k/6Q1/6K1/8/8/8/8/8 b - - 0 1",
        "4r2k/8/8/8/8/8/3PK3/8 w - - 0 1",
    ];

    #[derive(Default)]
    struct Coverage {
        positions: usize,
        checks: usize,
        double_checks: usize,
        pinned_positions: usize,
        en_passant_moves: usize,
        castling_moves: usize,
        promotion_moves: usize,
    }

    #[test]
    fn occupancy_filter_matches_reference_on_special_rules_and_random_play() {
        let mut coverage = Coverage::default();
        for fen in CURATED_FENS {
            let mut position = Position::from_fen(fen).expect("curated legality FEN is valid");
            compare_with_reference(&mut position, &mut coverage);
        }

        let mut random = SplitMix64(0xA6B4_9D27_C135_EE01);
        let mut position = Position::startpos();
        let mut game_ply = 0;
        while coverage.positions < 100_000 {
            let reference = compare_with_reference(&mut position, &mut coverage);
            if reference.is_empty() || game_ply == 191 {
                position = Position::startpos();
                game_ply = 0;
                continue;
            }
            let mv = reference.as_slice()[(random.next() as usize) % reference.len()];
            position.make_move(mv);
            game_ply += 1;
        }

        eprintln!(
            "legality oracle coverage: positions={} checks={} double_checks={} pinned_positions={} en_passant_moves={} castling_moves={} promotion_moves={}",
            coverage.positions,
            coverage.checks,
            coverage.double_checks,
            coverage.pinned_positions,
            coverage.en_passant_moves,
            coverage.castling_moves,
            coverage.promotion_moves,
        );
        assert!(coverage.positions >= 100_000);
        assert!(coverage.checks > 0, "corpus must contain checks");
        assert!(
            coverage.double_checks > 0,
            "corpus must contain double checks"
        );
        assert!(
            coverage.pinned_positions > 0,
            "corpus must contain absolute pins"
        );
        assert!(
            coverage.en_passant_moves > 0,
            "corpus must contain en-passant"
        );
        assert!(coverage.castling_moves > 0, "corpus must contain castling");
        assert!(
            coverage.promotion_moves > 0,
            "corpus must contain promotions"
        );
    }

    fn compare_with_reference(position: &mut Position, coverage: &mut Coverage) -> MoveList {
        let before = position.clone();
        let reference = generate_legal_reference(position);
        assert_eq!(
            *position, before,
            "reference generation must restore position"
        );
        let candidate = generate_legal(position);
        assert_eq!(has_legal_move(position), !reference.is_empty());
        assert_eq!(
            *position, before,
            "candidate generation must not mutate position"
        );
        assert_eq!(
            candidate.as_slice(),
            reference.as_slice(),
            "legality mismatch for {}",
            position.to_fen()
        );

        let color = position.side_to_move();
        if let Some(king) = single_square(position.pieces(color, PieceType::King)) {
            let checkers = attackers_to(
                position,
                king,
                color.opposite(),
                position.all_occupancy(),
                0,
            );
            if checkers != 0 {
                coverage.checks += 1;
            }
            if checkers.count_ones() > 1 {
                coverage.double_checks += 1;
            }
            if absolute_pins(position, color, king) != 0 {
                coverage.pinned_positions += 1;
            }
        }
        coverage.en_passant_moves += reference.iter().filter(|mv| mv.is_en_passant()).count();
        coverage.castling_moves += reference.iter().filter(|mv| mv.is_castle()).count();
        coverage.promotion_moves += reference.iter().filter(|mv| mv.is_promotion()).count();
        coverage.positions += 1;
        reference
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
