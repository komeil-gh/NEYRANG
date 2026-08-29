use neyrang::{
    chess::{Move, PieceType, Position, Square},
    search::{see, see_ge},
};

const VALUE: [i32; 6] = [100, 320, 330, 500, 900, 20_000];
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

fn legal_move(position: &mut Position, notation: &str) -> Move {
    position
        .find_legal_move(notation)
        .unwrap_or_else(|| panic!("{notation} must be legal in {}", position.to_fen()))
}

fn immediate_gain(position: &Position, mv: Move) -> i32 {
    let capture_square = if mv.is_en_passant() {
        Square::from_coords(mv.to().file(), mv.from().rank())
            .expect("an en-passant capture square is on the board")
    } else {
        mv.to()
    };
    let captured = position
        .piece_at(capture_square)
        .map_or(0, |(_, kind)| VALUE[kind.index()]);
    let promotion = mv.promotion().map_or(0, |kind| {
        VALUE[kind.index()] - VALUE[PieceType::Pawn.index()]
    });
    captured + promotion
}

fn reference_recaptures(position: &mut Position, target: Square) -> i32 {
    let moves = position.legal_moves();
    let mut best = 0;
    for &mv in moves.iter() {
        if mv.to() != target || !mv.is_capture() {
            continue;
        }
        let gain = immediate_gain(position, mv);
        let undo = position.make_move(mv);
        let score = gain - reference_recaptures(position, target);
        position.unmake_move(mv, undo);
        best = best.max(score);
    }
    best
}

fn reference_see(position: &Position, mv: Move) -> i32 {
    let immediate = immediate_gain(position, mv);
    let mut child = position.clone();
    child.make_move(mv);
    immediate - reference_recaptures(&mut child, mv.to())
}

fn assert_threshold_equivalence(position: &Position, mv: Move, exact: i32) {
    for threshold in FIXED_THRESHOLDS.into_iter().chain([
        exact.saturating_sub(1),
        exact,
        exact.saturating_add(1),
    ]) {
        assert_eq!(
            see_ge(position, mv, threshold),
            exact >= threshold,
            "threshold SEE mismatch for {mv} at {threshold} in {} (exact {exact})",
            position.to_fen()
        );
    }
}

fn assert_matches_reference(fen: &str, notation: &str) -> i32 {
    let mut position = Position::from_fen(fen).expect("SEE fixture must be valid");
    let mv = legal_move(&mut position, notation);
    let expected = reference_see(&position, mv);
    let actual = see(&position, mv);
    assert_eq!(actual, expected, "SEE mismatch for {notation} in {fen}");
    assert_threshold_equivalence(&position, mv, actual);
    actual
}

#[test]
fn see_values_an_undefended_pawn_capture() {
    let score = assert_matches_reference("4k3/8/8/3p4/4P3/8/8/4K3 w - - 0 1", "e4d5");
    assert_eq!(score, 100);
}

#[test]
fn see_values_a_defended_pawn_as_an_equal_exchange() {
    let score = assert_matches_reference("4k3/8/2p5/3p4/4P3/8/8/4K3 w - - 0 1", "e4d5");
    assert_eq!(score, 0);
}

#[test]
fn see_distinguishes_winning_and_losing_queen_captures() {
    let winning = assert_matches_reference("4k3/8/2p5/3q4/4P3/8/8/4K3 w - - 0 1", "e4d5");
    let losing = assert_matches_reference("4k3/8/2p5/3p4/4Q3/8/8/4K3 w - - 0 1", "e4d5");
    assert_eq!(winning, 800);
    assert_eq!(losing, -800);
}

#[test]
fn see_reveals_a_rook_xray_attacker() {
    let score = assert_matches_reference("3rk3/8/8/3p4/3R4/8/8/3RK3 w - - 0 1", "d4d5");
    assert_eq!(score, 100);
}

#[test]
fn see_reveals_bishop_and_queen_xray_attackers() {
    let bishop_score = assert_matches_reference("4k1b1/8/8/3p4/2B5/8/B7/4K3 w - - 0 1", "c4d5");
    let queen_score = assert_matches_reference("4k1b1/8/8/3p4/2B5/8/Q7/4K3 w - - 0 1", "c4d5");
    assert_eq!(bishop_score, 100);
    assert_eq!(queen_score, 100);
}

#[test]
fn see_accounts_for_capture_promotions_and_underpromotions() {
    let queen_score = assert_matches_reference("4k2r/6P1/8/8/8/8/8/4K3 w - - 0 1", "g7h8q");
    let knight_score = assert_matches_reference("4k2r/6P1/8/8/8/8/8/4K3 w - - 0 1", "g7h8n");
    assert_eq!(queen_score, 1_300);
    assert_eq!(knight_score, 720);
}

#[test]
fn see_accounts_for_quiet_promotions_for_both_colors() {
    let white = assert_matches_reference("4k3/P7/8/8/8/8/8/4K3 w - - 0 1", "a7a8q");
    let black = assert_matches_reference("4k3/8/8/8/8/8/p7/4K3 b - - 0 1", "a2a1q");
    assert_eq!(white, 800);
    assert_eq!(black, 800);
}

#[test]
fn see_removes_the_en_passant_pawn_before_finding_xrays() {
    let score = assert_matches_reference("4k3/4b3/8/3pP3/8/8/8/3RK3 w - d6 0 1", "e5d6");
    assert_eq!(score, 100);
}

#[test]
fn see_rejects_an_illegal_king_recapture() {
    let score = assert_matches_reference("8/4k3/4p3/3Q4/8/8/8/4R1K1 w - - 0 1", "d5e6");
    assert_eq!(score, 100);
}

#[test]
fn see_ignores_an_absolutely_pinned_attacker() {
    let score = assert_matches_reference("5k2/8/5n2/3p4/4Q3/8/8/5RK1 w - - 0 1", "e4d5");
    assert_eq!(score, 100);
}

#[test]
fn see_handles_multiple_attackers_and_defenders() {
    let score = assert_matches_reference("4k3/8/2p2n2/3p4/4PN2/8/8/4K3 w - - 0 1", "e4d5");
    assert_eq!(score, 0);
}

#[test]
fn threshold_see_preserves_the_existing_lva_choice_in_an_ambiguous_exchange() {
    let mut position =
        Position::from_fen("6k1/r2qb1p1/p1nP1p1r/1p3Q2/3P2N1/B3pP1p/P1P4P/2R1KBR1 w - - 2 32")
            .expect("ambiguous LVA fixture must be valid");
    let mv = legal_move(&mut position, "f5f6");
    let exact_lva = see(&position, mv);

    // NEYRANG's retained SEE follows one legal least-valuable-attacker sequence,
    // like conventional fast SEE. Exhaustively choosing among equal-valued
    // recapturers is a different semantic change and is outside Phase B.
    assert_eq!(exact_lva, -700);
    assert_eq!(reference_see(&position, mv), -790);
    assert_threshold_equivalence(&position, mv, exact_lva);
}

#[test]
fn threshold_see_matches_exact_see_over_thousands_of_legal_positions() {
    let mut position = Position::startpos();
    let mut state = 0xB0B1_5EE0_2026_0829_u64;
    let mut tactical_moves = 0_usize;

    for sample in 0..4_096 {
        if sample != 0 && sample % 128 == 0 {
            position = Position::startpos();
        }
        let legal = position.legal_moves();
        if legal.is_empty() {
            position = Position::startpos();
            continue;
        }

        for &mv in legal.iter() {
            let exact = see(&position, mv);
            assert_threshold_equivalence(&position, mv, exact);
            if mv.is_capture() || mv.is_promotion() {
                tactical_moves += 1;
            }
        }

        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        let advance = legal.as_slice()[(state as usize) % legal.len()];
        position.make_move(advance);
    }

    assert!(
        tactical_moves >= 4_096,
        "random corpus must exercise thousands of tactical moves, got {tactical_moves}"
    );
}
