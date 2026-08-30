//! Replaceable evaluation boundary. The first implementation is a deterministic
//! tapered handcrafted evaluator; NNUE can later implement the same contract.

mod classical;
mod pawns;
mod psqt;
#[cfg(any(test, feature = "eval-tools"))]
mod trace;

pub use classical::{TEMPO, evaluate};
#[cfg(feature = "eval-tools")]
pub use trace::{EvalTrace, TRACE_COLUMNS, TRACE_SCHEMA, trace};
