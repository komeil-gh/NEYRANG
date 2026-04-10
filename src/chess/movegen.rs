use super::{
    Bitboard, CastlingRights, Color, Move, MoveFlag, MoveList, PieceType, Position, Square, attacks,
};

pub(crate) fn generate_legal(position: &mut Position) -> MoveList {
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
    if attacks::pawn_attacks(by.opposite(), square) & position.pieces(by, PieceType::Pawn) != 0 {
        return true;
    }
    if attacks::knight_attacks(square) & position.pieces(by, PieceType::Knight) != 0 {
        return true;
    }
    if attacks::king_attacks(square) & position.pieces(by, PieceType::King) != 0 {
        return true;
    }
    let diagonal = position.pieces(by, PieceType::Bishop) | position.pieces(by, PieceType::Queen);
    if attacks::bishop_attacks(square, position.all_occupancy()) & diagonal != 0 {
        return true;
    }
    let orthogonal = position.pieces(by, PieceType::Rook) | position.pieces(by, PieceType::Queen);
    attacks::rook_attacks(square, position.all_occupancy()) & orthogonal != 0
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
