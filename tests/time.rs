use std::time::Duration;

use neyrang::search::time::TimeManager;

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
