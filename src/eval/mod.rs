//! Replaceable evaluation boundary. The first implementation is a deterministic
//! tapered handcrafted evaluator; NNUE can later implement the same contract.

mod classical;
mod pawns;
mod psqt;

pub use classical::{TEMPO, evaluate};
