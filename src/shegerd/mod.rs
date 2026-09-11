//! SHEGERD: reusable strength techniques used by REKHNE.
//!
//! This layer owns move ordering, history heuristics, and static exchange
//! evaluation. Keeping them outside the search driver gives every technique a
//! clear contract and a measurable right to remain in NEYRANG.

pub(crate) mod fischer;
pub mod history;
pub(crate) mod ordering;
mod policy;
mod see;

pub use see::{see, see_ge};
