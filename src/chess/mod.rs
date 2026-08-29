//! Architecture-independent chess rules and position representation.

pub mod attacks;
mod bitboard;
mod fen;
mod movegen;
mod mv;
mod perft;
mod piece;
mod position;
mod square;
mod state;
mod types;
pub mod zobrist;

pub use bitboard::Bitboard;
pub use fen::FenError;
pub use mv::{Move, MoveFlag, MoveList};
pub use perft::{divide, perft};
pub use piece::Piece;
pub use position::{CastlingRights, Position};
pub use square::Square;
pub use state::{NullUndoState, UndoState};
pub use types::{Color, PieceType};
