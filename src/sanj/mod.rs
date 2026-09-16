//! SANJ: NEYRANG's replaceable position-judgment boundary.
//!
//! Classical SANJ remains the deterministic fallback. The default `nnue`
//! feature adds the retained residual evaluator behind the same search contract.

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
/// UCI chooses the embedded retained network in a default build. The enum keeps
/// classical as its construction default for explicit tools and tests.
#[derive(Clone, Default)]
pub enum Evaluator {
    #[default]
    Classical,
    #[cfg(feature = "nnue")]
    Nnue {
        network: Arc<nnue::Network>,
        nnue_percent: u8,
    },
}

impl Evaluator {
    #[must_use]
    pub const fn classical() -> Self {
        Self::Classical
    }

    #[cfg(feature = "nnue")]
    #[must_use]
    pub fn nnue(network: nnue::Network) -> Self {
        Self::nnue_with_mix(network, 100)
    }

    #[cfg(feature = "nnue")]
    #[must_use]
    pub fn nnue_with_mix(network: nnue::Network, nnue_percent: u8) -> Self {
        assert!(nnue_percent <= 100);
        Self::Nnue {
            network: Arc::new(network),
            nnue_percent,
        }
    }

    #[must_use]
    pub fn evaluate(&self, position: &Position) -> i32 {
        match self {
            Self::Classical => evaluate(position),
            #[cfg(feature = "nnue")]
            Self::Nnue {
                network,
                nnue_percent,
            } => match nnue_percent {
                0 => evaluate(position),
                100 => network.evaluate(position),
                percent => blend(evaluate(position), network.evaluate(position), *percent),
            },
        }
    }

    #[cfg(feature = "nnue")]
    pub fn set_nnue_mix(&mut self, nnue_percent: u8) {
        assert!(nnue_percent <= 100);
        if let Self::Nnue {
            nnue_percent: active,
            ..
        } = self
        {
            *active = nnue_percent;
        }
    }

    #[cfg(feature = "nnue")]
    pub(crate) fn blend_accumulator_score(&self, position: &Position, nnue_score: i32) -> i32 {
        match self {
            Self::Classical => evaluate(position),
            Self::Nnue { nnue_percent, .. } => match nnue_percent {
                0 => evaluate(position),
                100 => nnue_score,
                percent => blend(evaluate(position), nnue_score, *percent),
            },
        }
    }

    #[cfg(feature = "nnue")]
    pub(crate) fn network(&self) -> Option<&nnue::Network> {
        match self {
            Self::Classical => None,
            Self::Nnue { network, .. } => Some(network),
        }
    }
}

#[cfg(feature = "nnue")]
fn blend(classical: i32, nnue: i32, nnue_percent: u8) -> i32 {
    let nnue_weight = i64::from(nnue_percent);
    ((i64::from(classical) * (100 - nnue_weight) + i64::from(nnue) * nnue_weight) / 100) as i32
}

#[cfg(all(test, feature = "nnue"))]
mod tests {
    use super::blend;

    #[test]
    fn bounded_mix_preserves_both_endpoints() {
        assert_eq!(blend(-300, 700, 0), -300);
        assert_eq!(blend(-300, 700, 25), -50);
        assert_eq!(blend(-300, 700, 100), 700);
    }
}
