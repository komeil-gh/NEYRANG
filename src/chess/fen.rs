use std::{error::Error, fmt, str::FromStr};

use super::{CastlingRights, Color, Piece, PieceType, Position, Square};

/// A recoverable error returned for malformed external FEN input.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FenError(String);

impl FenError {
    fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl fmt::Display for FenError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for FenError {}

pub(crate) fn parse(fen: &str) -> Result<Position, FenError> {
    let fields: Vec<&str> = fen.split_whitespace().collect();
    if fields.len() != 6 {
        return Err(FenError::new("FEN must contain exactly six fields"));
    }

    let mut position = Position::empty();
    parse_placement(fields[0], &mut position)?;
    for color in [Color::White, Color::Black] {
        if position.pieces[color.index()][PieceType::King.index()].count_ones() != 1 {
            return Err(FenError::new(
                "piece placement must contain exactly one king per side",
            ));
        }
    }
    position.side_to_move = match fields[1] {
        "w" => Color::White,
        "b" => Color::Black,
        _ => return Err(FenError::new("active color must be 'w' or 'b'")),
    };
    position.castling_rights = parse_castling(fields[2])?;
    position.en_passant = parse_en_passant(fields[3])?;
    position.halfmove_clock = fields[4]
        .parse()
        .map_err(|_| FenError::new("halfmove clock must be an unsigned 16-bit integer"))?;
    position.fullmove_number = fields[5]
        .parse()
        .map_err(|_| FenError::new("fullmove number must be an unsigned 16-bit integer"))?;
    if position.fullmove_number == 0 {
        return Err(FenError::new("fullmove number must be at least one"));
    }
    position.hash = super::zobrist::recompute(&position);
    Ok(position)
}

fn parse_placement(field: &str, position: &mut Position) -> Result<(), FenError> {
    let ranks: Vec<&str> = field.split('/').collect();
    if ranks.len() != 8 {
        return Err(FenError::new("piece placement must contain eight ranks"));
    }

    for (fen_rank, rank_text) in ranks.iter().enumerate() {
        let board_rank = 7 - fen_rank as u8;
        let mut file = 0_u8;
        for byte in rank_text.bytes() {
            if byte.is_ascii_digit() {
                let empty = byte - b'0';
                if empty == 0 || empty > 8 || file + empty > 8 {
                    return Err(FenError::new(
                        "invalid empty-square count in piece placement",
                    ));
                }
                file += empty;
                continue;
            }
            let piece = Piece::from_fen(byte)
                .ok_or_else(|| FenError::new("invalid piece character in piece placement"))?;
            if file >= 8 {
                return Err(FenError::new("rank contains more than eight squares"));
            }
            let square = Square::from_coords(file, board_rank)
                .ok_or_else(|| FenError::new("piece square is out of range"))?;
            position.place_piece(square, piece);
            file += 1;
        }
        if file != 8 {
            return Err(FenError::new(
                "each rank must describe exactly eight squares",
            ));
        }
    }
    Ok(())
}

fn parse_castling(field: &str) -> Result<CastlingRights, FenError> {
    if field == "-" {
        return Ok(CastlingRights::from_bits(0));
    }
    let mut bits = 0_u8;
    for byte in field.bytes() {
        let right = match byte {
            b'K' => CastlingRights::WHITE_KING,
            b'Q' => CastlingRights::WHITE_QUEEN,
            b'k' => CastlingRights::BLACK_KING,
            b'q' => CastlingRights::BLACK_QUEEN,
            _ => return Err(FenError::new("castling field may only contain KQkq or '-'")),
        };
        if bits & right != 0 {
            return Err(FenError::new("castling field contains a duplicate right"));
        }
        bits |= right;
    }
    Ok(CastlingRights::from_bits(bits))
}

fn parse_en_passant(field: &str) -> Result<Option<Square>, FenError> {
    if field == "-" {
        return Ok(None);
    }
    let square = Square::from_str(field).map_err(FenError::new)?;
    if square.rank() != 2 && square.rank() != 5 {
        return Err(FenError::new(
            "en-passant target must be on rank three or six",
        ));
    }
    Ok(Some(square))
}

pub(crate) fn serialize(position: &Position) -> String {
    let mut output = String::with_capacity(80);
    for rank in (0..8).rev() {
        let mut empty = 0_u8;
        for file in 0..8 {
            let square = Square::from_coords(file, rank).expect("board coordinates are valid");
            if let Some(piece) = position.mailbox[square.index()] {
                if empty != 0 {
                    output.push(char::from(b'0' + empty));
                    empty = 0;
                }
                output.push(char::from(piece.kind.fen_byte(piece.color)));
            } else {
                empty += 1;
            }
        }
        if empty != 0 {
            output.push(char::from(b'0' + empty));
        }
        if rank != 0 {
            output.push('/');
        }
    }

    output.push(' ');
    output.push(match position.side_to_move {
        Color::White => 'w',
        Color::Black => 'b',
    });
    output.push(' ');
    append_castling(position.castling_rights, &mut output);
    output.push(' ');
    match position.en_passant {
        Some(square) => output.push_str(&square.to_string()),
        None => output.push('-'),
    }
    output.push_str(&format!(
        " {} {}",
        position.halfmove_clock, position.fullmove_number
    ));
    output
}

fn append_castling(rights: CastlingRights, output: &mut String) {
    let before = output.len();
    for (right, symbol) in [
        (CastlingRights::WHITE_KING, 'K'),
        (CastlingRights::WHITE_QUEEN, 'Q'),
        (CastlingRights::BLACK_KING, 'k'),
        (CastlingRights::BLACK_QUEEN, 'q'),
    ] {
        if rights.contains(right) {
            output.push(symbol);
        }
    }
    if output.len() == before {
        output.push('-');
    }
}
