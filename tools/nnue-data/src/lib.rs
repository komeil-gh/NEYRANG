//! Lossless, replay-validated training-game data for NEYRANG's NNUE pipeline.

mod codec;
mod model;
pub mod selfplay;

pub use codec::{decode_games, encode_games};
pub use model::{DataError, Game, GameResult, ScoredMove};
