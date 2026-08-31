use neyrang::chess::{CastlingRights, Color, Move, PieceType, Position, Square};

use crate::{DataError, Game, GameResult, ScoredMove};

const HEADER_SIZE: usize = 32;
const RECORD_SIZE: usize = 4;
const MAX_GAME_PLIES: usize = 1024;
const NO_EN_PASSANT: u8 = 64;

pub fn decode_games(bytes: &[u8]) -> Result<Vec<Game>, DataError> {
    if bytes.is_empty() {
        return Err(DataError::NoGames);
    }
    let mut games = Vec::new();
    let mut offset = 0;

    while offset < bytes.len() {
        let game_offset = offset;
        let header = bytes
            .get(offset..offset + HEADER_SIZE)
            .ok_or(DataError::TruncatedHeader { offset })?;
        let occupancy = u64::from_le_bytes(header[0..8].try_into().expect("fixed header field"));
        if occupancy == 0 {
            return Err(DataError::UnsupportedExtension { offset });
        }

        let initial_position = decode_position(header, occupancy)?;
        let header_score = i16::from_le_bytes([header[28], header[29]]);
        let result =
            GameResult::from_byte(header[30]).ok_or(DataError::InvalidResult(header[30]))?;
        let extra = header[31];
        offset += HEADER_SIZE;

        let mut position = initial_position.clone();
        let mut moves = Vec::new();
        let mut terminated = false;
        while offset < bytes.len() {
            let record = bytes
                .get(offset..offset + RECORD_SIZE)
                .ok_or(DataError::TruncatedMove { offset })?;
            let encoded = u16::from_le_bytes([record[0], record[1]]);
            let score_cp = i16::from_le_bytes([record[2], record[3]]);
            offset += RECORD_SIZE;
            if encoded == 0 && score_cp == 0 {
                terminated = true;
                break;
            }
            if moves.len() == MAX_GAME_PLIES {
                return Err(DataError::TooManyMoves(moves.len() + 1));
            }
            let mv = decode_legal_move(&mut position, encoded, moves.len())?;
            position.make_move(mv);
            moves.push(ScoredMove { mv, score_cp });
        }
        if !terminated {
            return Err(DataError::MissingTerminator {
                offset: game_offset,
            });
        }
        games.push(Game {
            initial_position,
            header_score,
            result,
            extra,
            moves,
        });
    }

    Ok(games)
}

pub fn encode_games(games: &[Game]) -> Result<Vec<u8>, DataError> {
    if games.is_empty() {
        return Err(DataError::NoGames);
    }
    let capacity = games
        .iter()
        .map(|game| HEADER_SIZE + RECORD_SIZE * (game.moves.len() + 1))
        .sum();
    let mut bytes = Vec::with_capacity(capacity);

    for game in games {
        if game.moves.len() > MAX_GAME_PLIES {
            return Err(DataError::TooManyMoves(game.moves.len()));
        }
        bytes.extend_from_slice(&encode_position(game)?);

        let mut position = game.initial_position.clone();
        for (ply, entry) in game.moves.iter().enumerate() {
            let legal = position
                .legal_moves()
                .iter()
                .any(|candidate| *candidate == entry.mv);
            if !legal {
                return Err(DataError::IllegalMove {
                    ply,
                    encoded: entry.mv.raw(),
                });
            }
            let encoded = encode_move(entry.mv);
            bytes.extend_from_slice(&encoded.to_le_bytes());
            bytes.extend_from_slice(&entry.score_cp.to_le_bytes());
            position.make_move(entry.mv);
        }
        bytes.extend_from_slice(&[0; RECORD_SIZE]);
    }

    Ok(bytes)
}

fn decode_position(header: &[u8], occupancy: u64) -> Result<Position, DataError> {
    let piece_count = occupancy.count_ones();
    if piece_count > 32 {
        return Err(DataError::TooManyPieces(piece_count));
    }

    let mut board = [None; 64];
    let mut castling = [false; 4];
    let mut occupied = occupancy;
    for piece_index in 0..piece_count as usize {
        let square_index = occupied.trailing_zeros() as u8;
        occupied &= occupied - 1;
        let packed = header[8 + piece_index / 2];
        let code = if piece_index % 2 == 0 {
            packed & 0x0f
        } else {
            packed >> 4
        };
        let color = if code & 8 == 0 {
            Color::White
        } else {
            Color::Black
        };
        let kind_code = code & 7;
        let kind = match kind_code {
            0 => PieceType::Pawn,
            1 => PieceType::Knight,
            2 => PieceType::Bishop,
            3 | 6 => PieceType::Rook,
            4 => PieceType::Queen,
            5 => PieceType::King,
            _ => {
                return Err(DataError::InvalidPieceCode {
                    code,
                    square: square_index,
                });
            }
        };
        if kind_code == 6 {
            let right_index = match (color, square_index) {
                (Color::White, 7) => 0,
                (Color::White, 0) => 1,
                (Color::Black, 63) => 2,
                (Color::Black, 56) => 3,
                _ => {
                    return Err(DataError::InvalidCastlingRights(format!(
                        "unmoved rook marker on square index {square_index}"
                    )));
                }
            };
            castling[right_index] = true;
        }
        board[square_index as usize] = Some((color, kind));
    }

    let side = if header[24] & 0x80 == 0 { "w" } else { "b" };
    let ep_index = header[24] & 0x7f;
    let en_passant = if ep_index == NO_EN_PASSANT {
        "-".to_owned()
    } else {
        Square::from_index(ep_index)
            .ok_or_else(|| DataError::InvalidPosition(format!("en-passant square {ep_index}")))?
            .to_string()
    };
    let rights = castling_string(castling);
    let board_fen = board_to_fen(&board);
    let fullmove = u16::from_le_bytes([header[26], header[27]]);
    let fen = format!(
        "{board_fen} {side} {rights} {en_passant} {} {fullmove}",
        header[25]
    );
    Position::from_fen(&fen).map_err(|error| DataError::InvalidPosition(error.to_string()))
}

fn encode_position(game: &Game) -> Result<[u8; HEADER_SIZE], DataError> {
    let position = &game.initial_position;
    if position.halfmove_clock() > u8::MAX as u16 {
        return Err(DataError::HalfmoveClockOutOfRange(
            position.halfmove_clock(),
        ));
    }
    validate_castling(position)?;

    let occupancy = position.all_occupancy();
    let piece_count = occupancy.count_ones();
    if piece_count > 32 {
        return Err(DataError::TooManyPieces(piece_count));
    }
    if occupancy == 0 {
        return Err(DataError::InvalidPosition(
            "position has no pieces".to_owned(),
        ));
    }

    let mut header = [0_u8; HEADER_SIZE];
    header[0..8].copy_from_slice(&occupancy.to_le_bytes());
    let mut occupied = occupancy;
    for piece_index in 0..piece_count as usize {
        let square_index = occupied.trailing_zeros() as u8;
        occupied &= occupied - 1;
        let square = Square::from_index(square_index).expect("occupied bit is a board square");
        let (color, kind) = position
            .piece_at(square)
            .ok_or_else(|| DataError::InvalidPosition(format!("empty occupied square {square}")))?;
        let mut kind_code = kind.index() as u8;
        if kind == PieceType::Rook && is_castling_rook(position, color, square) {
            kind_code = 6;
        }
        let code = kind_code | if color == Color::Black { 8 } else { 0 };
        if piece_index % 2 == 0 {
            header[8 + piece_index / 2] = code;
        } else {
            header[8 + piece_index / 2] |= code << 4;
        }
    }

    let ep = position
        .en_passant()
        .map_or(NO_EN_PASSANT, |square| square.index() as u8);
    header[24] = ep
        | if position.side_to_move() == Color::Black {
            0x80
        } else {
            0
        };
    header[25] = position.halfmove_clock() as u8;
    header[26..28].copy_from_slice(&position.fullmove_number().to_le_bytes());
    header[28..30].copy_from_slice(&game.header_score.to_le_bytes());
    header[30] = game.result.to_byte();
    header[31] = game.extra;
    Ok(header)
}

fn decode_legal_move(position: &mut Position, encoded: u16, ply: usize) -> Result<Move, DataError> {
    let from_index = (encoded & 0x3f) as u8;
    let packed_to = ((encoded >> 6) & 0x3f) as u8;
    let move_type = encoded >> 14;
    let promotion_code = ((encoded >> 12) & 3) as u8;
    if move_type != 3 && promotion_code != 0 {
        return Err(DataError::NonCanonicalMove { ply, encoded });
    }
    let to_index = if move_type == 2 {
        match (from_index, packed_to) {
            (4, 7) => 6,
            (4, 0) => 2,
            (60, 63) => 62,
            (60, 56) => 58,
            _ => return Err(DataError::IllegalMove { ply, encoded }),
        }
    } else {
        packed_to
    };
    let promotion = if move_type == 3 {
        Some(match promotion_code {
            0 => PieceType::Knight,
            1 => PieceType::Bishop,
            2 => PieceType::Rook,
            3 => PieceType::Queen,
            _ => unreachable!("two-bit promotion code"),
        })
    } else {
        None
    };

    position
        .legal_moves()
        .iter()
        .copied()
        .find(|candidate| {
            candidate.from().index() == from_index as usize
                && candidate.to().index() == to_index as usize
                && candidate.promotion() == promotion
                && match move_type {
                    0 => {
                        !candidate.is_castle()
                            && !candidate.is_en_passant()
                            && !candidate.is_promotion()
                    }
                    1 => candidate.is_en_passant(),
                    2 => candidate.is_castle(),
                    3 => candidate.is_promotion(),
                    _ => false,
                }
        })
        .ok_or(DataError::IllegalMove { ply, encoded })
}

fn encode_move(mv: Move) -> u16 {
    let from = mv.from().index() as u16;
    if mv.is_castle() {
        let rook_square = match mv.to() {
            Square::G1 => Square::H1,
            Square::C1 => Square::A1,
            Square::G8 => Square::H8,
            Square::C8 => Square::A8,
            _ => unreachable!("NEYRANG castling destinations are standard"),
        };
        return from | ((rook_square.index() as u16) << 6) | (2 << 14);
    }
    if let Some(piece) = mv.promotion() {
        let promotion_code = match piece {
            PieceType::Knight => 0,
            PieceType::Bishop => 1,
            PieceType::Rook => 2,
            PieceType::Queen => 3,
            PieceType::Pawn | PieceType::King => unreachable!("invalid promotion piece"),
        };
        return from | ((mv.to().index() as u16) << 6) | (promotion_code << 12) | (3 << 14);
    }
    let move_type = if mv.is_en_passant() { 1 } else { 0 };
    from | ((mv.to().index() as u16) << 6) | (move_type << 14)
}

fn validate_castling(position: &Position) -> Result<(), DataError> {
    let rights = position.castling_rights();
    for (right, king, rook, color, name) in [
        (
            CastlingRights::WHITE_KING,
            Square::E1,
            Square::H1,
            Color::White,
            "K",
        ),
        (
            CastlingRights::WHITE_QUEEN,
            Square::E1,
            Square::A1,
            Color::White,
            "Q",
        ),
        (
            CastlingRights::BLACK_KING,
            Square::E8,
            Square::H8,
            Color::Black,
            "k",
        ),
        (
            CastlingRights::BLACK_QUEEN,
            Square::E8,
            Square::A8,
            Color::Black,
            "q",
        ),
    ] {
        if rights.contains(right)
            && (position.piece_at(king) != Some((color, PieceType::King))
                || position.piece_at(rook) != Some((color, PieceType::Rook)))
        {
            return Err(DataError::InvalidCastlingRights(format!(
                "right {name} lacks its standard king or rook"
            )));
        }
    }
    Ok(())
}

fn is_castling_rook(position: &Position, color: Color, square: Square) -> bool {
    let rights = position.castling_rights();
    matches!(
        (color, square),
        (Color::White, Square::H1) if rights.contains(CastlingRights::WHITE_KING)
    ) || matches!(
        (color, square),
        (Color::White, Square::A1) if rights.contains(CastlingRights::WHITE_QUEEN)
    ) || matches!(
        (color, square),
        (Color::Black, Square::H8) if rights.contains(CastlingRights::BLACK_KING)
    ) || matches!(
        (color, square),
        (Color::Black, Square::A8) if rights.contains(CastlingRights::BLACK_QUEEN)
    )
}

fn castling_string(rights: [bool; 4]) -> String {
    let mut value = String::new();
    for (enabled, symbol) in rights.into_iter().zip(['K', 'Q', 'k', 'q']) {
        if enabled {
            value.push(symbol);
        }
    }
    if value.is_empty() {
        value.push('-');
    }
    value
}

fn board_to_fen(board: &[Option<(Color, PieceType)>; 64]) -> String {
    let mut fen = String::new();
    for rank in (0..8).rev() {
        if rank != 7 {
            fen.push('/');
        }
        let mut empty = 0;
        for file in 0..8 {
            match board[rank * 8 + file] {
                None => empty += 1,
                Some((color, kind)) => {
                    if empty != 0 {
                        fen.push(char::from(b'0' + empty));
                        empty = 0;
                    }
                    let symbol = match kind {
                        PieceType::Pawn => 'p',
                        PieceType::Knight => 'n',
                        PieceType::Bishop => 'b',
                        PieceType::Rook => 'r',
                        PieceType::Queen => 'q',
                        PieceType::King => 'k',
                    };
                    fen.push(if color == Color::White {
                        symbol.to_ascii_uppercase()
                    } else {
                        symbol
                    });
                }
            }
        }
        if empty != 0 {
            fen.push(char::from(b'0' + empty));
        }
    }
    fen
}
