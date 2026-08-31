use neyrang::chess::Position;
use neyrang_nnue_data::{Game, GameResult, ScoredMove, decode_games, encode_games};

fn game_from_line(fen: &str, line: &[(&str, i16)], result: GameResult) -> Game {
    let initial_position = Position::from_fen(fen).expect("test FEN is valid");
    let mut position = initial_position.clone();
    let mut moves = Vec::new();
    for (notation, score_cp) in line {
        let mv = position
            .find_legal_move(notation)
            .unwrap_or_else(|| panic!("{notation} is legal in {}", position.to_fen()));
        moves.push(ScoredMove {
            mv,
            score_cp: *score_cp,
        });
        position.make_move(mv);
    }
    Game {
        initial_position,
        header_score: -1234,
        result,
        extra: 0xa5,
        moves,
    }
}

#[test]
fn special_moves_and_metadata_roundtrip_losslessly() {
    let games = vec![
        game_from_line(
            "r3k2r/8/8/8/8/8/8/R3K2R w KQkq - 7 42",
            &[("e1g1", 15), ("e8c8", -22)],
            GameResult::Draw,
        ),
        game_from_line(
            "4k3/8/8/3pP3/8/8/8/4K3 w - d6 0 1",
            &[("e5d6", 87)],
            GameResult::WhiteWin,
        ),
        game_from_line(
            "4k3/P7/8/8/8/8/8/4K3 w - - 0 1",
            &[("a7a8n", 311)],
            GameResult::BlackWin,
        ),
    ];

    let encoded = encode_games(&games).expect("special-move games encode");
    let decoded = decode_games(&encoded).expect("special-move games decode");

    assert_eq!(decoded, games);
    assert_eq!(encode_games(&decoded).unwrap(), encoded);
}

#[test]
fn concatenated_games_keep_their_boundaries() {
    let games = vec![
        game_from_line(
            Position::STARTPOS_FEN,
            &[("e2e4", 10)],
            GameResult::WhiteWin,
        ),
        game_from_line(
            Position::STARTPOS_FEN,
            &[("d2d4", -8), ("g8f6", 4)],
            GameResult::Draw,
        ),
    ];

    let decoded = decode_games(&encode_games(&games).unwrap()).unwrap();

    assert_eq!(decoded, games);
}
