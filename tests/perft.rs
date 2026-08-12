use neyrang::chess::{Position, perft};

#[test]
fn starting_position_perft_matches_reference_counts() {
    let expected = [1, 20, 400, 8_902, 197_281];
    let mut position = Position::startpos();

    for (depth, nodes) in expected.into_iter().enumerate() {
        assert_eq!(perft(&mut position, depth as u8), nodes, "depth {depth}");
    }
}

#[test]
fn kiwipete_perft_covers_castling_checks_and_pins() {
    let expected = [1, 48, 2_039, 97_862, 4_085_603];
    let mut position =
        Position::from_fen("r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1")
            .expect("Kiwipete FEN is valid");

    let mut root_moves: Vec<String> = position
        .legal_moves()
        .iter()
        .map(ToString::to_string)
        .collect();
    root_moves.sort();
    assert_eq!(root_moves.len(), 48, "root moves: {root_moves:?}");

    for (depth, nodes) in expected.into_iter().enumerate() {
        assert_eq!(perft(&mut position, depth as u8), nodes, "depth {depth}");
    }
}

#[test]
fn supplied_kiwipete_variant_has_its_reference_root_count() {
    let mut position =
        Position::from_fen("r3k2r/p1ppqpb1/bn2pnp1/2pP4/1p2P3/2N2N2/PPQBBPPP/R3K2R w KQkq - 0 1")
            .expect("the supplied variant FEN is valid");

    // Independently cross-checked with python-chess 1.11.2. This is not the
    // canonical 48-move Kiwipete layout despite often being labelled as such.
    assert_eq!(perft(&mut position, 1), 45);
}

#[test]
fn rook_and_pawn_endgame_perft_covers_en_passant_tactics() {
    let expected = [1, 14, 191, 2_812, 43_238, 674_624];
    let mut position = Position::from_fen("8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1")
        .expect("the standard perft endgame FEN is valid");

    for (depth, nodes) in expected.into_iter().enumerate() {
        assert_eq!(perft(&mut position, depth as u8), nodes, "depth {depth}");
    }
}
