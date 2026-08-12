use neyrang::chess::{Color, PieceType, Position, Square};

#[test]
fn starting_position_has_the_expected_state() {
    let position = Position::startpos();

    assert_eq!(position.side_to_move(), Color::White);
    assert_eq!(
        position.piece_at(Square::E1),
        Some((Color::White, PieceType::King))
    );
    assert_eq!(
        position.piece_at(Square::D8),
        Some((Color::Black, PieceType::Queen))
    );
    assert_eq!(position.to_fen(), Position::STARTPOS_FEN);
}

#[test]
fn fen_without_both_kings_is_rejected() {
    let error = Position::from_fen("8/8/8/8/8/8/8/4K3 w - - 0 1")
        .expect_err("a position without a black king must be rejected");

    assert!(error.to_string().contains("king"));
}

#[test]
fn complex_fen_round_trip_preserves_irreversible_state() {
    let fen = "r3k2r/ppp2ppp/2n5/3pp3/3PP3/2N5/PPP2PPP/R3K2R b KQkq e3 17 42";
    let position = Position::from_fen(fen).expect("the fixture is valid");

    assert_eq!(position.to_fen(), fen);
    assert_eq!(position.en_passant(), Some("e3".parse().unwrap()));
    assert_eq!(position.halfmove_clock(), 17);
    assert_eq!(position.fullmove_number(), 42);
}
