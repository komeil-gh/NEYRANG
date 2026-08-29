use crate::chess::{Color, Move, PieceType, Position, Square, attacks};

const VALUE: [i32; 6] = [100, 320, 330, 500, 900, 20_000];

struct ExchangeState {
    color: Color,
    target: Square,
    target_kind: PieceType,
    pieces: [[u64; 6]; 2],
    occupancy: u64,
    immediate: i32,
}

trait SeeObserver {
    fn exchange_step(&mut self) {}
    fn early_exit(&mut self) {}
}

#[cfg(not(feature = "stats"))]
struct NoSeeObserver;

#[cfg(not(feature = "stats"))]
impl SeeObserver for NoSeeObserver {}

#[cfg(feature = "stats")]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct SeeWork {
    pub exchange_steps: u64,
    pub early_exit: bool,
}

#[cfg(feature = "stats")]
impl SeeObserver for SeeWork {
    fn exchange_step(&mut self) {
        self.exchange_steps += 1;
    }

    fn early_exit(&mut self) {
        self.early_exit = true;
    }
}

/// Static exchange evaluation on the destination square. Positive values favor
/// the side making `mv`; either side may decline a continuation capture.
pub fn see(position: &Position, mv: Move) -> i32 {
    #[cfg(feature = "stats")]
    {
        see_with_work(position, mv).0
    }
    #[cfg(not(feature = "stats"))]
    {
        see_observed(position, mv, &mut NoSeeObserver)
    }
}

fn see_observed<O: SeeObserver>(position: &Position, mv: Move, observer: &mut O) -> i32 {
    let Some(exchange) = prepare_exchange(position, mv) else {
        return 0;
    };
    exchange.immediate
        - recapture_gain(
            exchange.color.opposite(),
            exchange.target,
            exchange.target_kind,
            exchange.pieces,
            exchange.occupancy,
            observer,
        )
}

fn prepare_exchange(position: &Position, mv: Move) -> Option<ExchangeState> {
    if !mv.is_capture() && !mv.is_promotion() {
        return None;
    }
    let (color, moving_kind) = position.piece_at(mv.from())?;
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
    Some(ExchangeState {
        color,
        target: mv.to(),
        target_kind,
        pieces,
        occupancy,
        immediate,
    })
}

/// Return whether the static exchange result reaches `threshold`.
///
/// This is a null-window query over the same legal least-valuable-attacker
/// sequence as [`see`]. It can prove the threshold without resolving the exact
/// exchange score, but its Boolean result is identical to `see >= threshold`.
pub fn see_ge(position: &Position, mv: Move, threshold: i32) -> bool {
    #[cfg(feature = "stats")]
    {
        see_ge_with_work(position, mv, threshold).0
    }
    #[cfg(not(feature = "stats"))]
    {
        see_ge_observed(position, mv, threshold, &mut NoSeeObserver)
    }
}

fn see_ge_observed<O: SeeObserver>(
    position: &Position,
    mv: Move,
    threshold: i32,
    observer: &mut O,
) -> bool {
    let Some(exchange) = prepare_exchange(position, mv) else {
        return 0 >= threshold;
    };

    // SEE = immediate - opponent_recapture_gain. The query therefore asks
    // whether the opponent's non-negative gain is at most this bound.
    let opponent_bound = i64::from(exchange.immediate) - i64::from(threshold);
    if opponent_bound < 0 {
        observer.early_exit();
        return false;
    }
    !recapture_gain_ge(
        exchange.color.opposite(),
        exchange.target,
        exchange.target_kind,
        exchange.pieces,
        exchange.occupancy,
        opponent_bound + 1,
        observer,
    )
}

#[cfg(feature = "stats")]
pub(crate) fn see_with_work(position: &Position, mv: Move) -> (i32, SeeWork) {
    let mut work = SeeWork::default();
    let value = see_observed(position, mv, &mut work);
    (value, work)
}

#[cfg(feature = "stats")]
pub(crate) fn see_ge_with_work(position: &Position, mv: Move, threshold: i32) -> (bool, SeeWork) {
    let mut work = SeeWork::default();
    let passes = see_ge_observed(position, mv, threshold, &mut work);
    (passes, work)
}

fn recapture_gain<O: SeeObserver>(
    color: Color,
    target: Square,
    target_kind: PieceType,
    pieces: [[u64; 6]; 2],
    occupancy: u64,
    observer: &mut O,
) -> i32 {
    let attackers = attackers_to(target, occupancy, &pieces) & color_occupancy(color, &pieces);
    let Some((attacker_kind, from)) =
        least_valuable_attacker(color, target, target_kind, attackers, &pieces, occupancy)
    else {
        return 0;
    };
    observer.exchange_step();

    let (next_pieces, next_occupancy, promoted_kind) = recapture_state(
        color,
        target,
        target_kind,
        attacker_kind,
        from,
        pieces,
        occupancy,
    );

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
            observer,
        );
    gain.max(0)
}

fn recapture_gain_ge<O: SeeObserver>(
    color: Color,
    target: Square,
    target_kind: PieceType,
    pieces: [[u64; 6]; 2],
    occupancy: u64,
    threshold: i64,
    observer: &mut O,
) -> bool {
    // Every side may decline the exchange, so recapture gain is non-negative.
    if threshold <= 0 {
        observer.early_exit();
        return true;
    }

    let attackers = attackers_to(target, occupancy, &pieces) & color_occupancy(color, &pieces);
    let Some((attacker_kind, from)) =
        least_valuable_attacker(color, target, target_kind, attackers, &pieces, occupancy)
    else {
        return false;
    };
    observer.exchange_step();

    let (next_pieces, next_occupancy, promoted_kind) = recapture_state(
        color,
        target,
        target_kind,
        attacker_kind,
        from,
        pieces,
        occupancy,
    );
    let promotion_gain = if promoted_kind != attacker_kind {
        VALUE[promoted_kind.index()] - VALUE[attacker_kind.index()]
    } else {
        0
    };
    let capture_gain = i64::from(VALUE[target_kind.index()] + promotion_gain);
    if capture_gain < threshold {
        observer.early_exit();
        return false;
    }

    // G = max(0, capture_gain - next_G). For a positive threshold t,
    // G >= t iff next_G <= capture_gain - t. Integer scores let the latter
    // become the negation of next_G >= capture_gain - t + 1.
    !recapture_gain_ge(
        color.opposite(),
        target,
        promoted_kind,
        next_pieces,
        next_occupancy,
        capture_gain - threshold + 1,
        observer,
    )
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
    target: Square,
    target_kind: PieceType,
    attackers: u64,
    pieces: &[[u64; 6]; 2],
    occupancy: u64,
) -> Option<(PieceType, Square)> {
    for kind in PieceType::ALL {
        let mut candidates = attackers & pieces[color.index()][kind.index()];
        while candidates != 0 {
            let from = Square::from_index(candidates.trailing_zeros() as u8)
                .expect("attacker bit is a valid square");
            candidates &= candidates - 1;
            let (next_pieces, next_occupancy, _) =
                recapture_state(color, target, target_kind, kind, from, *pieces, occupancy);
            if king_is_safe(color, &next_pieces, next_occupancy) {
                return Some((kind, from));
            }
        }
    }
    None
}

fn recapture_state(
    color: Color,
    target: Square,
    target_kind: PieceType,
    attacker_kind: PieceType,
    from: Square,
    mut pieces: [[u64; 6]; 2],
    occupancy: u64,
) -> ([[u64; 6]; 2], u64, PieceType) {
    pieces[color.opposite().index()][target_kind.index()] &= !target.bit();
    pieces[color.index()][attacker_kind.index()] &= !from.bit();
    let promoted_kind = if attacker_kind == PieceType::Pawn && matches!(target.rank(), 0 | 7) {
        PieceType::Queen
    } else {
        attacker_kind
    };
    pieces[color.index()][promoted_kind.index()] |= target.bit();
    (pieces, occupancy & !from.bit(), promoted_kind)
}

fn king_is_safe(color: Color, pieces: &[[u64; 6]; 2], occupancy: u64) -> bool {
    let king = pieces[color.index()][PieceType::King.index()];
    if king.count_ones() != 1 {
        return false;
    }
    let king_square =
        Square::from_index(king.trailing_zeros() as u8).expect("a king bit is a valid square");
    attackers_to(king_square, occupancy, pieces) & color_occupancy(color.opposite(), pieces) == 0
}

fn color_occupancy(color: Color, pieces: &[[u64; 6]; 2]) -> u64 {
    pieces[color.index()]
        .iter()
        .copied()
        .fold(0, |all, bitboard| all | bitboard)
}
