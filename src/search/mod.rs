//! Deterministic single-thread search. Parallel search is intentionally deferred
//! until this implementation is correct and measurable.

mod driver;
pub mod history;
mod limits;
mod ordering;
mod see;
pub mod time;
pub mod tt;

pub use driver::{
    MAX_PLY, SearchInfo, SearchResult, SearchStatistics, Searcher, VALUE_DRAW, VALUE_INFINITE,
    VALUE_MATE,
};
pub use limits::SearchLimits;
pub use see::{see, see_ge};
