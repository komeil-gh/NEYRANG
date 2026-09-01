//! SANJ: NEYRANG's replaceable position-judgment boundary.
//!
//! The current implementation is a deterministic tapered handcrafted evaluator;
//! NNUE can later implement the same contract.

mod classical;
mod pawns;
mod psqt;
#[cfg(any(test, feature = "sanj-tools"))]
mod trace;

pub use classical::{TEMPO, evaluate};
#[cfg(feature = "sanj-tools")]
pub use trace::{EvalTrace, TRACE_COLUMNS, TRACE_SCHEMA, trace};
