use crate::chess::{Color, Move, PieceType, Position, Square, attacks};

const VALUE: [i32; 6] = [100, 320, 330, 500, 900, 20_000];

/// Static exchange evaluation on the destination square. Positive values favor
/// the side making `mv`; either side may decline a continuation capture.
pub fn see(position: &Position, mv: Move) -> i32 {
    if !mv.is_capture() && !mv.is_promotion() {
        return 0;
    }
    let Some((color, moving_kind)) = position.piece_at(mv.from()) else {
        return 0;
    };
    let mut pieces = [[0_u64; 6]; 2];
    for side in [Color::White, Color::Black] {
        for kind in PieceType::ALL {
            pieces[side.index()][kind.index()] = position.pieces(side, kind);
        }
    }
    let capture_square = if mv.is_en_passant() {
        Square::from_coords(mv.to().file(), mv.from().rank())
            .expect("legal en-passant capture square is valid")
    } else {
        mv.to()
    };
    let captured_kind = position.piece_at(capture_square).map(|(_, kind)| kind);
    let mut occupancy = position.all_occupancy();
    pieces[color.index()][moving_kind.index()] &= !mv.from().bit();
    occupancy &= !mv.from().bit();
    if let Some((captured_color, kind)) = position.piece_at(capture_square) {
        pieces[captured_color.index()][kind.index()] &= !capture_square.bit();
        occupancy &= !capture_square.bit();
    }
    let target_kind = mv.promotion().unwrap_or(moving_kind);
    pieces[color.index()][target_kind.index()] |= mv.to().bit();
    occupancy |= mv.to().bit();

    let promotion_gain = mv.promotion().map_or(0, |kind| {
        VALUE[kind.index()] - VALUE[PieceType::Pawn.index()]
    });
    let immediate = captured_kind.map_or(0, |kind| VALUE[kind.index()]) + promotion_gain;
    immediate - recapture_gain(color.opposite(), mv.to(), target_kind, pieces, occupancy)
}

fn recapture_gain(
    color: Color,
    target: Square,
    target_kind: PieceType,
    pieces: [[u64; 6]; 2],
    occupancy: u64,
) -> i32 {
    let attackers = attackers_to(target, occupancy, &pieces) & color_occupancy(color, &pieces);
    let Some((attacker_kind, from)) = least_valuable_attacker(color, attackers, &pieces) else {
        return 0;
    };

    let mut next_pieces = pieces;
    next_pieces[color.opposite().index()][target_kind.index()] &= !target.bit();
    next_pieces[color.index()][attacker_kind.index()] &= !from.bit();
    let promoted_kind = if attacker_kind == PieceType::Pawn && matches!(target.rank(), 0 | 7) {
        PieceType::Queen
    } else {
        attacker_kind
    };
    next_pieces[color.index()][promoted_kind.index()] |= target.bit();
    let next_occupancy = occupancy & !from.bit();

    if attacker_kind == PieceType::King
        && attackers_to(target, next_occupancy, &next_pieces)
            & color_occupancy(color.opposite(), &next_pieces)
            != 0
    {
        return 0;
    }

    let promotion_gain = if promoted_kind != attacker_kind {
        VALUE[promoted_kind.index()] - VALUE[attacker_kind.index()]
    } else {
        0
    };
    let gain = VALUE[target_kind.index()] + promotion_gain
        - recapture_gain(
            color.opposite(),
            target,
            promoted_kind,
            next_pieces,
            next_occupancy,
        );
    gain.max(0)
}

fn attackers_to(target: Square, occupancy: u64, pieces: &[[u64; 6]; 2]) -> u64 {
    let white_pawns = attacks::pawn_attacks(Color::Black, target)
        & pieces[Color::White.index()][PieceType::Pawn.index()];
    let black_pawns = attacks::pawn_attacks(Color::White, target)
        & pieces[Color::Black.index()][PieceType::Pawn.index()];
    let knights = attacks::knight_attacks(target)
        & (pieces[0][PieceType::Knight.index()] | pieces[1][PieceType::Knight.index()]);
    let kings = attacks::king_attacks(target)
        & (pieces[0][PieceType::King.index()] | pieces[1][PieceType::King.index()]);
    let diagonal = attacks::bishop_attacks(target, occupancy)
        & (pieces[0][PieceType::Bishop.index()]
            | pieces[1][PieceType::Bishop.index()]
            | pieces[0][PieceType::Queen.index()]
            | pieces[1][PieceType::Queen.index()]);
    let orthogonal = attacks::rook_attacks(target, occupancy)
        & (pieces[0][PieceType::Rook.index()]
            | pieces[1][PieceType::Rook.index()]
            | pieces[0][PieceType::Queen.index()]
            | pieces[1][PieceType::Queen.index()]);
    white_pawns | black_pawns | knights | kings | diagonal | orthogonal
}

fn least_valuable_attacker(
    color: Color,
    attackers: u64,
    pieces: &[[u64; 6]; 2],
) -> Option<(PieceType, Square)> {
    for kind in PieceType::ALL {
        let candidates = attackers & pieces[color.index()][kind.index()];
        if candidates != 0 {
            let square = Square::from_index(candidates.trailing_zeros() as u8)
                .expect("attacker bit is a valid square");
            return Some((kind, square));
        }
    }
    None
}

fn color_occupancy(color: Color, pieces: &[[u64; 6]; 2]) -> u64 {
    pieces[color.index()]
        .iter()
        .copied()
        .fold(0, |all, bitboard| all | bitboard)
}
