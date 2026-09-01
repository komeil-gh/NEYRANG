use std::time::Duration;

use neyrang::rekhne::time::TimeManager;

#[test]
fn clock_budget_keeps_a_hard_deadline_inside_remaining_time() {
    let budget = TimeManager::allocate(
        Duration::from_secs(10),
        Duration::from_millis(100),
        Some(20),
        Duration::from_millis(15),
    );

    assert!(budget.soft > Duration::ZERO);
    assert!(budget.soft < budget.hard);
    assert!(budget.hard <= Duration::from_millis(9_985));
}

#[test]
fn clock_budget_stops_immediately_when_overhead_consumes_the_clock() {
    let budget = TimeManager::allocate(
        Duration::from_millis(6),
        Duration::from_millis(2),
        None,
        Duration::from_millis(10),
    );

    assert_eq!(budget.soft, Duration::ZERO);
    assert_eq!(budget.hard, Duration::ZERO);
}
