use std::time::Duration;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TimeBudget {
    pub soft: Duration,
    pub hard: Duration,
}

pub struct TimeManager;

impl TimeManager {
    /// Allocate conservatively from the active clock. If communication
    /// overhead consumes the remaining time, return an emergency zero budget.
    pub fn allocate(
        remaining: Duration,
        increment: Duration,
        moves_to_go: Option<u32>,
        overhead: Duration,
    ) -> TimeBudget {
        if remaining <= overhead {
            return TimeBudget {
                soft: Duration::ZERO,
                hard: Duration::ZERO,
            };
        }
        let usable = remaining.saturating_sub(overhead);
        let horizon = moves_to_go.unwrap_or(30).clamp(1, 50);
        let base = usable / horizon;
        let increment_share = increment * 3 / 4;
        let mut soft = (base + increment_share).min(usable * 7 / 10);
        soft = soft.max(Duration::from_millis(1).min(usable));
        let hard = (soft * 3).max(soft + Duration::from_millis(1)).min(usable);
        if soft >= hard && hard > Duration::from_micros(1) {
            soft = hard * 3 / 4;
        }
        TimeBudget { soft, hard }
    }
}
