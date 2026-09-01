//! REKHNE: NEYRANG's search strategy and search-control infrastructure.

mod driver;
mod limits;
mod parallel;
pub mod time;
pub mod tt;

pub use driver::{
    MAX_PLY, SearchInfo, SearchResult, SearchStatistics, Searcher, VALUE_DRAW, VALUE_INFINITE,
    VALUE_MATE,
};
pub use limits::SearchLimits;
pub(crate) use parallel::{ParallelOptions, search_parallel_with_evaluator};
