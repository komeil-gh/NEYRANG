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
    assert!(budget.hard <= Duration::from_millis(9_970));
}

#[test]
fn clock_budget_reserves_two_response_windows() {
    let budget = TimeManager::allocate(
        Duration::from_millis(205),
        Duration::from_millis(5),
        None,
        Duration::from_millis(100),
    );

    assert!(budget.hard <= Duration::from_millis(5));
}

#[test]
fn clock_budget_stops_when_two_overhead_windows_consume_the_clock() {
    let budget = TimeManager::allocate(
        Duration::from_millis(199),
        Duration::from_millis(5),
        None,
        Duration::from_millis(100),
    );

    assert_eq!(budget.soft, Duration::ZERO);
    assert_eq!(budget.hard, Duration::ZERO);
}
