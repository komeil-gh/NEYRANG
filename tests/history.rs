use neyrang::{
    chess::{Color, Move, MoveFlag, Square},
    shegerd::history::HistoryTable,
};

#[test]
fn quiet_history_rewards_cutoff_moves_and_saturates() {
    let mut history = HistoryTable::default();
    let mv = Move::new(Square::E2, Square::E4, MoveFlag::DoublePawnPush);

    for _ in 0..10_000 {
        history.reward(Color::White, mv, 8);
    }
    let rewarded = history.score(Color::White, mv);
    assert!(rewarded > 0);
    assert!(rewarded <= HistoryTable::MAX_SCORE);

    history.penalize(Color::White, mv, 8);
    assert!(history.score(Color::White, mv) < rewarded);
    assert_eq!(history.score(Color::Black, mv), 0);
}
