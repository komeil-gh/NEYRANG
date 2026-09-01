//! SANJ: NEYRANG's replaceable position-judgment boundary.
//!
//! Classical SANJ is the deterministic default. The non-default `nnue` feature
//! adds an opt-in evaluator that implements the same search-facing contract.

#[cfg(feature = "nnue")]
use std::sync::Arc;

use crate::chess::Position;

mod classical;
#[cfg(feature = "nnue")]
pub mod nnue;
mod pawns;
mod psqt;
#[cfg(any(test, feature = "sanj-tools"))]
mod trace;

pub use classical::{TEMPO, evaluate};
#[cfg(feature = "sanj-tools")]
pub use trace::{EvalTrace, TRACE_COLUMNS, TRACE_SCHEMA, trace};

/// Search-owned choice of SANJ implementation.
///
/// Classical evaluation remains the default. An NNUE network can only be
/// selected in an explicit `nnue` feature build after its artifact has passed
/// the fail-closed loader.
#[derive(Clone, Default)]
pub enum Evaluator {
    #[default]
    Classical,
    #[cfg(feature = "nnue")]
    Nnue(Arc<nnue::Network>),
}

impl Evaluator {
    #[must_use]
    pub const fn classical() -> Self {
        Self::Classical
    }

    #[cfg(feature = "nnue")]
    #[must_use]
    pub fn nnue(network: nnue::Network) -> Self {
        Self::Nnue(Arc::new(network))
    }

    #[must_use]
    pub fn evaluate(&self, position: &Position) -> i32 {
        match self {
            Self::Classical => evaluate(position),
            #[cfg(feature = "nnue")]
            Self::Nnue(network) => network.evaluate(position),
        }
    }

    #[cfg(feature = "nnue")]
    pub(crate) fn network(&self) -> Option<&nnue::Network> {
        match self {
            Self::Classical => None,
            Self::Nnue(network) => Some(network),
        }
    }
}
