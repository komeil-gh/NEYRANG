use crate::chess::{Color, Move, PieceType, Position, Square, attacks};

const VALUE: [i32; 6] = [100, 320, 330, 500, 900, 20_000];

#[derive(Clone, Copy)]
struct ExchangeBoard {
    pieces: [[u64; 6]; 2],
    by_color: [u64; 2],
    occupancy: u64,
}

struct ExchangeState {
    color: Color,
    target: Square,
    target_kind: PieceType,
    board: ExchangeBoard,
    attackers: u64,
    immediate: i32,
}

struct Recapture {
    attacker_kind: PieceType,
    from: Square,
    promoted_kind: PieceType,
    board: ExchangeBoard,
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
            exchange.board,
            exchange.attackers,
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
    let mut by_color = [
        position.occupancy(Color::White),
        position.occupancy(Color::Black),
    ];
    let capture_square = if mv.is_en_passant() {
        Square::from_coords(mv.to().file(), mv.from().rank())
            .expect("legal en-passant capture square is valid")
    } else {
        mv.to()
    };
    let captured_kind = position.piece_at(capture_square).map(|(_, kind)| kind);
    let mut occupancy = position.all_occupancy();
    pieces[color.index()][moving_kind.index()] &= !mv.from().bit();
    by_color[color.index()] &= !mv.from().bit();
    occupancy &= !mv.from().bit();
    if let Some((captured_color, kind)) = position.piece_at(capture_square) {
        pieces[captured_color.index()][kind.index()] &= !capture_square.bit();
        by_color[captured_color.index()] &= !capture_square.bit();
        occupancy &= !capture_square.bit();
    }
    let target_kind = mv.promotion().unwrap_or(moving_kind);
    pieces[color.index()][target_kind.index()] |= mv.to().bit();
    by_color[color.index()] |= mv.to().bit();
    occupancy |= mv.to().bit();

    let promotion_gain = mv.promotion().map_or(0, |kind| {
        VALUE[kind.index()] - VALUE[PieceType::Pawn.index()]
    });
    let immediate = captured_kind.map_or(0, |kind| VALUE[kind.index()]) + promotion_gain;
    let board = ExchangeBoard {
        pieces,
        by_color,
        occupancy,
    };
    Some(ExchangeState {
        color,
        target: mv.to(),
        target_kind,
        attackers: attackers_to(mv.to(), occupancy, &pieces) & occupancy,
        board,
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
        exchange.board,
        exchange.attackers,
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
    board: ExchangeBoard,
    attackers: u64,
    observer: &mut O,
) -> i32 {
    let candidates = attackers & board.by_color[color.index()];
    let Some(recapture) = least_valuable_attacker(color, target, target_kind, candidates, &board)
    else {
        return 0;
    };
    observer.exchange_step();

    let next_attackers = reveal_attackers(target, attackers, recapture.from, &recapture.board);
    let promotion_gain = if recapture.promoted_kind != recapture.attacker_kind {
        VALUE[recapture.promoted_kind.index()] - VALUE[recapture.attacker_kind.index()]
    } else {
        0
    };
    let gain = VALUE[target_kind.index()] + promotion_gain
        - recapture_gain(
            color.opposite(),
            target,
            recapture.promoted_kind,
            recapture.board,
            next_attackers,
            observer,
        );
    gain.max(0)
}

fn recapture_gain_ge<O: SeeObserver>(
    color: Color,
    target: Square,
    target_kind: PieceType,
    board: ExchangeBoard,
    attackers: u64,
    threshold: i64,
    observer: &mut O,
) -> bool {
    // Every side may decline the exchange, so recapture gain is non-negative.
    if threshold <= 0 {
        observer.early_exit();
        return true;
    }

    let candidates = attackers & board.by_color[color.index()];
    let Some(recapture) = least_valuable_attacker(color, target, target_kind, candidates, &board)
    else {
        return false;
    };
    observer.exchange_step();

    let next_attackers = reveal_attackers(target, attackers, recapture.from, &recapture.board);
    let promotion_gain = if recapture.promoted_kind != recapture.attacker_kind {
        VALUE[recapture.promoted_kind.index()] - VALUE[recapture.attacker_kind.index()]
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
        recapture.promoted_kind,
        recapture.board,
        next_attackers,
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
    board: &ExchangeBoard,
) -> Option<Recapture> {
    for kind in PieceType::ALL {
        let mut candidates = attackers & board.pieces[color.index()][kind.index()];
        while candidates != 0 {
            let from = Square::from_index(candidates.trailing_zeros() as u8)
                .expect("attacker bit is a valid square");
            candidates &= candidates - 1;
            let (next_board, promoted_kind) =
                recapture_state(color, target, target_kind, kind, from, *board);
            if king_is_safe(color, &next_board) {
                return Some(Recapture {
                    attacker_kind: kind,
                    from,
                    promoted_kind,
                    board: next_board,
                });
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
    mut board: ExchangeBoard,
) -> (ExchangeBoard, PieceType) {
    let enemy = color.opposite();
    board.pieces[enemy.index()][target_kind.index()] &= !target.bit();
    board.by_color[enemy.index()] &= !target.bit();
    board.pieces[color.index()][attacker_kind.index()] &= !from.bit();
    board.by_color[color.index()] &= !from.bit();
    let promoted_kind = if attacker_kind == PieceType::Pawn && matches!(target.rank(), 0 | 7) {
        PieceType::Queen
    } else {
        attacker_kind
    };
    board.pieces[color.index()][promoted_kind.index()] |= target.bit();
    board.by_color[color.index()] |= target.bit();
    board.occupancy &= !from.bit();
    (board, promoted_kind)
}

fn king_is_safe(color: Color, board: &ExchangeBoard) -> bool {
    let king = board.pieces[color.index()][PieceType::King.index()];
    if king.count_ones() != 1 {
        return false;
    }
    let king_square =
        Square::from_index(king.trailing_zeros() as u8).expect("a king bit is a valid square");
    let enemy = color.opposite().index();
    let enemy_pawns = board.pieces[enemy][PieceType::Pawn.index()];
    if attacks::pawn_attacks(color, king_square) & enemy_pawns != 0 {
        return false;
    }
    if attacks::knight_attacks(king_square) & board.pieces[enemy][PieceType::Knight.index()] != 0 {
        return false;
    }
    if attacks::king_attacks(king_square) & board.pieces[enemy][PieceType::King.index()] != 0 {
        return false;
    }
    let diagonal = board.pieces[enemy][PieceType::Bishop.index()]
        | board.pieces[enemy][PieceType::Queen.index()];
    if attacks::bishop_attacks(king_square, board.occupancy) & diagonal != 0 {
        return false;
    }
    let orthogonal = board.pieces[enemy][PieceType::Rook.index()]
        | board.pieces[enemy][PieceType::Queen.index()];
    attacks::rook_attacks(king_square, board.occupancy) & orthogonal == 0
}

fn reveal_attackers(target: Square, attackers: u64, vacated: Square, board: &ExchangeBoard) -> u64 {
    let mut revealed = attackers & board.occupancy;
    let file_distance = target.file().abs_diff(vacated.file());
    let rank_distance = target.rank().abs_diff(vacated.rank());
    if file_distance == rank_distance {
        let diagonal = board.pieces[0][PieceType::Bishop.index()]
            | board.pieces[1][PieceType::Bishop.index()]
            | board.pieces[0][PieceType::Queen.index()]
            | board.pieces[1][PieceType::Queen.index()];
        revealed |= attacks::bishop_attacks(target, board.occupancy) & diagonal;
    } else if file_distance == 0 || rank_distance == 0 {
        let orthogonal = board.pieces[0][PieceType::Rook.index()]
            | board.pieces[1][PieceType::Rook.index()]
            | board.pieces[0][PieceType::Queen.index()]
            | board.pieces[1][PieceType::Queen.index()];
        revealed |= attacks::rook_attacks(target, board.occupancy) & orthogonal;
    }
    revealed & board.occupancy
}

#[cfg(test)]
mod legacy {
    use super::*;

    struct ExchangeState {
        color: Color,
        target: Square,
        target_kind: PieceType,
        pieces: [[u64; 6]; 2],
        occupancy: u64,
        immediate: i32,
    }

    pub(super) fn see_observed<O: SeeObserver>(
        position: &Position,
        mv: Move,
        observer: &mut O,
    ) -> i32 {
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

    pub(super) fn see_ge_observed<O: SeeObserver>(
        position: &Position,
        mv: Move,
        threshold: i32,
        observer: &mut O,
    ) -> bool {
        let Some(exchange) = prepare_exchange(position, mv) else {
            return 0 >= threshold;
        };
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
        attackers_to(king_square, occupancy, pieces) & color_occupancy(color.opposite(), pieces)
            == 0
    }

    fn color_occupancy(color: Color, pieces: &[[u64; 6]; 2]) -> u64 {
        pieces[color.index()]
            .iter()
            .copied()
            .fold(0, |all, bitboard| all | bitboard)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXED_THRESHOLDS: [i32; 19] = [
        i32::MIN,
        -30_000,
        -20_000,
        -1_000,
        -900,
        -500,
        -330,
        -320,
        -100,
        -1,
        0,
        1,
        100,
        320,
        330,
        500,
        900,
        30_000,
        i32::MAX,
    ];
    const CURATED_FENS: [&str; 6] = [
        "4k3/4b3/8/3pP3/8/8/8/3RK3 w - d6 0 1",
        "4k2r/6P1/8/8/8/8/8/4K3 w - - 0 1",
        "4r2k/8/8/8/1b6/8/8/4K3 w - - 0 1",
        "8/4k3/4p3/3Q4/8/8/8/4R1K1 w - - 0 1",
        "5k2/8/5n2/3p4/4Q3/8/8/5RK1 w - - 0 1",
        "6k1/r2qb1p1/p1nP1p1r/1p3Q2/3P2N1/B3pP1p/P1P4P/2R1KBR1 w - - 2 32",
    ];

    #[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
    struct TestWork {
        exchange_steps: u64,
        early_exit: bool,
    }

    impl SeeObserver for TestWork {
        fn exchange_step(&mut self) {
            self.exchange_steps += 1;
        }

        fn early_exit(&mut self) {
            self.early_exit = true;
        }
    }

    #[derive(Default)]
    struct Coverage {
        positions: usize,
        tactical_moves: usize,
        captures: usize,
        promotions: usize,
        en_passant: usize,
        threshold_queries: usize,
    }

    #[test]
    fn incremental_state_matches_the_immutable_see_oracle() {
        let mut coverage = Coverage::default();
        for fen in CURATED_FENS {
            let mut position = Position::from_fen(fen).expect("curated SEE FEN is valid");
            compare_position(&mut position, &mut coverage);
        }

        let mut random = SplitMix64(0x5EE0_AA47_2026_0830);
        let mut position = Position::startpos();
        let mut game_ply = 0;
        while coverage.positions < 100_000 || coverage.tactical_moves < 100_000 {
            let legal = compare_position(&mut position, &mut coverage);
            if legal.is_empty() || game_ply == 191 {
                position = Position::startpos();
                game_ply = 0;
                continue;
            }
            let mv = legal.as_slice()[(random.next() as usize) % legal.len()];
            position.make_move(mv);
            game_ply += 1;
        }

        eprintln!(
            "SEE oracle coverage: positions={} tactical_moves={} captures={} promotions={} en_passant={} threshold_queries={}",
            coverage.positions,
            coverage.tactical_moves,
            coverage.captures,
            coverage.promotions,
            coverage.en_passant,
            coverage.threshold_queries,
        );
        assert!(coverage.positions >= 100_000);
        assert!(coverage.tactical_moves >= 100_000);
        assert!(coverage.captures > 0);
        assert!(coverage.promotions > 0);
        assert!(coverage.en_passant > 0);
    }

    fn compare_position(
        position: &mut Position,
        coverage: &mut Coverage,
    ) -> crate::chess::MoveList {
        let legal = position.legal_moves();
        for &mv in legal.iter() {
            if !mv.is_capture() && !mv.is_promotion() {
                continue;
            }
            compare_move(position, mv, coverage);
        }
        coverage.positions += 1;
        legal
    }

    fn compare_move(position: &Position, mv: Move, coverage: &mut Coverage) {
        let mut candidate_work = TestWork::default();
        let candidate = see_observed(position, mv, &mut candidate_work);
        let mut oracle_work = TestWork::default();
        let oracle = legacy::see_observed(position, mv, &mut oracle_work);
        assert_eq!(
            (candidate, candidate_work),
            (oracle, oracle_work),
            "exact SEE mismatch for {mv} in {}",
            position.to_fen()
        );

        for threshold in FIXED_THRESHOLDS.into_iter().chain([
            oracle.saturating_sub(1),
            oracle,
            oracle.saturating_add(1),
        ]) {
            let mut candidate_work = TestWork::default();
            let candidate = see_ge_observed(position, mv, threshold, &mut candidate_work);
            let mut oracle_work = TestWork::default();
            let oracle = legacy::see_ge_observed(position, mv, threshold, &mut oracle_work);
            assert_eq!(
                (candidate, candidate_work),
                (oracle, oracle_work),
                "threshold SEE mismatch for {mv} at {threshold} in {}",
                position.to_fen()
            );
            coverage.threshold_queries += 1;
        }

        coverage.tactical_moves += 1;
        coverage.captures += usize::from(mv.is_capture());
        coverage.promotions += usize::from(mv.is_promotion());
        coverage.en_passant += usize::from(mv.is_en_passant());
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
