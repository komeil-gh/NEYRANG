use neyrang::chess::{Move, MoveFlag, Position, Square};
use neyrang_nnue_data::{DataError, Game, GameResult, ScoredMove, decode_games, encode_games};

fn empty_startpos_game() -> Game {
    Game {
        initial_position: Position::startpos(),
        header_score: 0,
        result: GameResult::Draw,
        extra: 0,
        moves: Vec::new(),
    }
}

#[test]
fn rejects_empty_files_and_empty_game_sets() {
    assert_eq!(decode_games(&[]), Err(DataError::NoGames));
    assert_eq!(encode_games(&[]), Err(DataError::NoGames));
}

#[test]
fn rejects_truncated_headers_records_and_missing_terminators() {
    assert_eq!(
        decode_games(&[1; 31]),
        Err(DataError::TruncatedHeader { offset: 0 })
    );

    let mut missing_terminator = encode_games(&[empty_startpos_game()]).unwrap();
    missing_terminator.truncate(32);
    assert_eq!(
        decode_games(&missing_terminator),
        Err(DataError::MissingTerminator { offset: 0 })
    );

    let mut truncated_record = missing_terminator;
    truncated_record.push(1);
    assert_eq!(
        decode_games(&truncated_record),
        Err(DataError::TruncatedMove { offset: 32 })
    );
}

#[test]
fn rejects_extensions_reserved_piece_codes_and_invalid_results() {
    assert_eq!(
        decode_games(&[0; 32]),
        Err(DataError::UnsupportedExtension { offset: 0 })
    );

    let encoded = encode_games(&[empty_startpos_game()]).unwrap();
    let mut invalid_piece = encoded.clone();
    invalid_piece[8] = (invalid_piece[8] & 0xf0) | 7;
    assert_eq!(
        decode_games(&invalid_piece),
        Err(DataError::InvalidPieceCode { code: 7, square: 0 })
    );

    let mut invalid_result = encoded;
    invalid_result[30] = 3;
    assert_eq!(
        decode_games(&invalid_result),
        Err(DataError::InvalidResult(3))
    );
}

#[test]
fn rejects_illegal_moves_on_encode_and_decode() {
    let illegal = Move::new(Square::E2, Square::E5, MoveFlag::Quiet);
    let mut game = empty_startpos_game();
    game.moves.push(ScoredMove {
        mv: illegal,
        score_cp: 0,
    });
    assert!(matches!(
        encode_games(&[game]),
        Err(DataError::IllegalMove { ply: 0, .. })
    ));

    let mut bytes = encode_games(&[empty_startpos_game()]).unwrap();
    bytes.truncate(32);
    let packed_e2e5 = 12_u16 | (36_u16 << 6);
    bytes.extend_from_slice(&packed_e2e5.to_le_bytes());
    bytes.extend_from_slice(&0_i16.to_le_bytes());
    bytes.extend_from_slice(&[0; 4]);
    assert_eq!(
        decode_games(&bytes),
        Err(DataError::IllegalMove {
            ply: 0,
            encoded: packed_e2e5,
        })
    );
}

#[test]
fn rejects_nonzero_promotion_bits_on_an_ordinary_move() {
    let mut bytes = encode_games(&[empty_startpos_game()]).unwrap();
    bytes.truncate(32);
    let noncanonical_e2e4 = 12_u16 | (28_u16 << 6) | (1_u16 << 12);
    bytes.extend_from_slice(&noncanonical_e2e4.to_le_bytes());
    bytes.extend_from_slice(&0_i16.to_le_bytes());
    bytes.extend_from_slice(&[0; 4]);

    assert_eq!(
        decode_games(&bytes),
        Err(DataError::NonCanonicalMove {
            ply: 0,
            encoded: noncanonical_e2e4,
        })
    );
}

#[test]
fn rejects_state_that_cannot_be_represented_losslessly() {
    let mut game = empty_startpos_game();
    game.initial_position =
        Position::from_fen("4k3/8/8/8/8/8/8/4K3 w - - 256 1").expect("clock fits NEYRANG");
    assert_eq!(
        encode_games(&[game]),
        Err(DataError::HalfmoveClockOutOfRange(256))
    );

    let mut game = empty_startpos_game();
    game.initial_position = Position::from_fen("4k3/8/8/8/8/8/8/4K3 w K - 0 1")
        .expect("FEN can retain an inconsistent castling flag for validation");
    assert!(matches!(
        encode_games(&[game]),
        Err(DataError::InvalidCastlingRights(_))
    ));
}

#[test]
fn rejects_games_beyond_the_defensive_ply_limit() {
    let mut game = empty_startpos_game();
    game.moves = vec![
        ScoredMove {
            mv: Move::new(Square::G1, Square::F3, MoveFlag::Quiet),
            score_cp: 0,
        };
        1025
    ];

    assert_eq!(encode_games(&[game]), Err(DataError::TooManyMoves(1025)));
}
