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
fn clock_budget_enters_emergency_when_overhead_consumes_the_clock() {
    let budget = TimeManager::allocate(
        Duration::from_millis(100),
        Duration::from_millis(5),
        None,
        Duration::from_millis(100),
    );

    assert_eq!(budget.soft, Duration::ZERO);
    assert_eq!(budget.hard, Duration::ZERO);
}

#[test]
fn clock_budget_preserves_ordinary_allocation_above_one_overhead() {
    let budget = TimeManager::allocate(
        Duration::from_millis(101),
        Duration::from_millis(5),
        None,
        Duration::from_millis(100),
    );

    assert!(budget.soft > Duration::ZERO);
    assert!(budget.hard > budget.soft);
    assert!(budget.hard <= Duration::from_millis(1));
}
