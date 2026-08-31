use std::{error::Error, fmt};

use neyrang::chess::{Move, Position};

/// Game outcome encoded from White's point of view.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GameResult {
    BlackWin,
    Draw,
    WhiteWin,
}

impl GameResult {
    pub(crate) const fn to_byte(self) -> u8 {
        match self {
            Self::BlackWin => 0,
            Self::Draw => 1,
            Self::WhiteWin => 2,
        }
    }

    pub(crate) const fn from_byte(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::BlackWin),
            1 => Some(Self::Draw),
            2 => Some(Self::WhiteWin),
            _ => None,
        }
    }
}

/// A legal move paired with the white-relative search score of its parent position.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ScoredMove {
    pub mv: Move,
    pub score_cp: i16,
}

/// One initial position and its contiguous, replayable game trajectory.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Game {
    pub initial_position: Position,
    pub header_score: i16,
    pub result: GameResult,
    pub extra: u8,
    pub moves: Vec<ScoredMove>,
}

/// A stable error boundary for malformed or non-lossless data.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DataError {
    NoGames,
    TruncatedHeader { offset: usize },
    TruncatedMove { offset: usize },
    MissingTerminator { offset: usize },
    UnsupportedExtension { offset: usize },
    InvalidPieceCode { code: u8, square: u8 },
    InvalidResult(u8),
    InvalidPosition(String),
    InvalidCastlingRights(String),
    HalfmoveClockOutOfRange(u16),
    TooManyPieces(u32),
    TooManyMoves(usize),
    NonCanonicalMove { ply: usize, encoded: u16 },
    IllegalMove { ply: usize, encoded: u16 },
}

impl fmt::Display for DataError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoGames => formatter.write_str("game corpus contains no games"),
            Self::TruncatedHeader { offset } => {
                write!(formatter, "truncated 32-byte game header at byte {offset}")
            }
            Self::TruncatedMove { offset } => {
                write!(formatter, "truncated move/score record at byte {offset}")
            }
            Self::MissingTerminator { offset } => {
                write!(
                    formatter,
                    "game starting at byte {offset} has no terminator"
                )
            }
            Self::UnsupportedExtension { offset } => {
                write!(
                    formatter,
                    "unsupported Viriformat extension at byte {offset}"
                )
            }
            Self::InvalidPieceCode { code, square } => {
                write!(
                    formatter,
                    "invalid piece code {code} on square index {square}"
                )
            }
            Self::InvalidResult(value) => write!(formatter, "invalid WDL result byte {value}"),
            Self::InvalidPosition(message) => {
                write!(formatter, "invalid packed position: {message}")
            }
            Self::InvalidCastlingRights(message) => {
                write!(
                    formatter,
                    "castling rights are not losslessly representable: {message}"
                )
            }
            Self::HalfmoveClockOutOfRange(value) => {
                write!(
                    formatter,
                    "halfmove clock {value} exceeds the format maximum of 255"
                )
            }
            Self::TooManyPieces(value) => {
                write!(
                    formatter,
                    "position contains {value} pieces; format maximum is 32"
                )
            }
            Self::TooManyMoves(value) => {
                write!(
                    formatter,
                    "game contains {value} plies; safety maximum is 1024"
                )
            }
            Self::NonCanonicalMove { ply, encoded } => {
                write!(
                    formatter,
                    "encoded move 0x{encoded:04x} has non-zero reserved bits at ply {ply}"
                )
            }
            Self::IllegalMove { ply, encoded } => {
                write!(
                    formatter,
                    "encoded move 0x{encoded:04x} is illegal at ply {ply}"
                )
            }
        }
    }
}

impl Error for DataError {}
