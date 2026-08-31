use neyrang_nnue_data::{GameResult, decode_games, encode_games};

const OFFICIAL_VIRIFORMAT_EXAMPLE: &[u8] = &[
    0xff, 0xff, 0x00, 0x00, 0x00, 0x00, 0xff, 0xff, // occupancy
    0x16, 0x42, 0x25, 0x61, 0x00, 0x00, 0x00, 0x00, // white pieces
    0x88, 0x88, 0x88, 0x88, 0x9e, 0xca, 0xad, 0xe9, // black pieces
    0x40, 0x00, 0x01, 0x00, 0x00, 0x00, 0x02, 0x00, // state, score, result
    0x0c, 0x07, 0x0a, 0x00, // e2e4, +10
    0x34, 0x09, 0x14, 0x00, // e7e5, +20
    0xc3, 0x09, 0xe2, 0xff, // d1h5, -30
    0x3c, 0x0d, 0xff, 0x7f, // e8e7, +32767
    0x27, 0x09, 0xff, 0x7f, // h5e5, +32767
    0x00, 0x00, 0x00, 0x00, // game terminator
];

#[test]
fn official_viriformat_example_decodes_and_reencodes_byte_exact() {
    let games = decode_games(OFFICIAL_VIRIFORMAT_EXAMPLE).expect("official fixture is valid");

    assert_eq!(games.len(), 1);
    let game = &games[0];
    assert_eq!(
        game.initial_position.to_fen(),
        neyrang::chess::Position::STARTPOS_FEN
    );
    assert_eq!(game.result, GameResult::WhiteWin);
    assert_eq!(game.moves.len(), 5);
    assert_eq!(
        game.moves
            .iter()
            .map(|entry| (entry.mv.to_string(), entry.score_cp))
            .collect::<Vec<_>>(),
        [
            ("e2e4".to_string(), 10),
            ("e7e5".to_string(), 20),
            ("d1h5".to_string(), -30),
            ("e8e7".to_string(), i16::MAX),
            ("h5e5".to_string(), i16::MAX),
        ]
    );

    assert_eq!(
        encode_games(&games).expect("decoded fixture can be encoded"),
        OFFICIAL_VIRIFORMAT_EXAMPLE
    );
}
