use std::time::Duration;

/// Normalized limits consumed by the searcher after UCI parsing/time allocation.
#[derive(Clone, Debug, Default)]
pub struct SearchLimits {
    pub depth: Option<u8>,
    pub nodes: Option<u64>,
    pub soft_time: Option<Duration>,
    pub hard_time: Option<Duration>,
    pub infinite: bool,
}

impl SearchLimits {
    pub const fn depth(depth: u8) -> Self {
        Self {
            depth: Some(depth),
            nodes: None,
            soft_time: None,
            hard_time: None,
            infinite: false,
        }
    }

    pub const fn nodes(nodes: u64) -> Self {
        Self {
            depth: None,
            nodes: Some(nodes),
            soft_time: None,
            hard_time: None,
            infinite: false,
        }
    }

    pub const fn infinite() -> Self {
        Self {
            depth: None,
            nodes: None,
            soft_time: None,
            hard_time: None,
            infinite: true,
        }
    }
}
