//! SHEGERD: reusable strength techniques used by REKHNE.
//!
//! This layer owns move ordering, history heuristics, and static exchange
//! evaluation. Keeping them outside the search driver gives every technique a
//! clear contract and a measurable right to remain in NEYRANG.

pub(crate) mod fischer;
pub mod history;
pub(crate) mod ordering;
#[cfg(feature = "policy")]
pub mod policy;
mod see;

pub use see::{see, see_ge};

#[cfg(feature = "policy")]
use std::sync::Arc;

use crate::chess::{Move, Position, Square};

#[derive(Clone, Default)]
pub struct MovePolicy {
    #[cfg(feature = "policy")]
    network: Option<Arc<policy::Network>>,
}

impl MovePolicy {
    pub(crate) const NONE: Self = Self {
        #[cfg(feature = "policy")]
        network: None,
    };

    #[cfg(feature = "policy")]
    #[must_use]
    pub fn from_network(network: policy::Network) -> Self {
        Self {
            network: Some(Arc::new(network)),
        }
    }

    #[inline]
    pub(crate) fn score(
        &self,
        position: &Position,
        mv: Move,
        previous_to: Option<Square>,
        exchange: i32,
    ) -> i32 {
        #[cfg(feature = "policy")]
        if let Some(network) = &self.network {
            return network.score(position, mv, previous_to, exchange);
        }
        let _ = (position, mv, previous_to, exchange);
        0
    }
}
