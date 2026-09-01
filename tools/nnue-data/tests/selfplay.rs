use neyrang::chess::{Color, Position};
use neyrang_nnue_data::{
    GameResult, ScoredMove,
    selfplay::{
        AcceptedGame, CompletionReason, GameAttempt, RejectionReason, SearchSample,
        SelfPlaySummary, record_game_with, white_relative_score,
    },
};

fn scripted_sample(position: &mut Position, notation: &str, score_cp: i32) -> SearchSample {
    SearchSample {
        best_move: position
            .find_legal_move(notation)
            .unwrap_or_else(|| panic!("{notation} must be legal in {}", position.to_fen())),
        score_cp,
    }
}

#[test]
fn scores_are_white_relative_and_saturate_without_overflow() {
    assert_eq!(white_relative_score(Color::White, 137), 137);
    assert_eq!(white_relative_score(Color::Black, 137), -137);
    assert_eq!(white_relative_score(Color::White, i32::MAX), i16::MAX);
    assert_eq!(white_relative_score(Color::Black, i32::MIN), i16::MAX);
}

#[test]
fn checkmate_game_records_each_parent_score_and_white_relative_wdl() {
    let mut script = [("f2f3", 11), ("e7e5", 22), ("g2g4", -33), ("d8h4", 29_999)].into_iter();

    let attempt = record_game_with(Position::startpos(), 32, |position, _history| {
        let (notation, score_cp) = script.next()?;
        Some(scripted_sample(position, notation, score_cp))
    });

    let GameAttempt::Accepted(accepted) = attempt else {
        panic!("forced checkmate must be accepted");
    };
    let AcceptedGame { game, completion } = *accepted;
    assert_eq!(completion, CompletionReason::Checkmate);
    assert_eq!(game.result, GameResult::BlackWin);
    assert_eq!(game.header_score, 0);
    assert_eq!(
        game.moves
            .iter()
            .map(|entry| entry.score_cp)
            .collect::<Vec<_>>(),
        vec![11, -22, -33, -29_999]
    );
}

#[test]
fn threefold_is_accepted_but_an_unfinished_max_ply_game_is_rejected() {
    let mut repetition = [
        ("g1f3", 0),
        ("g8f6", 0),
        ("f3g1", 0),
        ("f6g8", 0),
        ("g1f3", 0),
        ("g8f6", 0),
        ("f3g1", 0),
        ("f6g8", 0),
    ]
    .into_iter();
    let repeated = record_game_with(Position::startpos(), 16, |position, _history| {
        let (notation, score_cp) = repetition.next()?;
        Some(scripted_sample(position, notation, score_cp))
    });
    let GameAttempt::Accepted(accepted) = repeated else {
        panic!("threefold game must be accepted");
    };
    let AcceptedGame { game, completion } = *accepted;
    assert_eq!(completion, CompletionReason::ThreefoldRepetition);
    assert_eq!(game.result, GameResult::Draw);
    assert_eq!(game.moves.len(), 8);

    let unfinished = record_game_with(Position::startpos(), 1, |position, _history| {
        Some(scripted_sample(position, "e2e4", 5))
    });
    assert_eq!(
        unfinished,
        GameAttempt::Rejected(RejectionReason::MaximumPlies)
    );
}

#[test]
fn summary_keeps_attempt_accept_reject_and_completion_reason_counts() {
    let mut initial_position = Position::startpos();
    let mv = initial_position
        .find_legal_move("e2e4")
        .expect("e2e4 is legal in startpos");
    let accepted = GameAttempt::Accepted(Box::new(AcceptedGame {
        game: neyrang_nnue_data::Game {
            initial_position,
            header_score: 0,
            result: GameResult::Draw,
            extra: 0,
            moves: vec![ScoredMove { mv, score_cp: 7 }],
        },
        completion: CompletionReason::FiftyMoveRule,
    }));
    let rejected = GameAttempt::Rejected(RejectionReason::MaximumPlies);
    let mut summary = SelfPlaySummary::default();

    summary.record(&accepted);
    summary.record(&rejected);

    assert_eq!(summary.attempted_games, 2);
    assert_eq!(summary.accepted_games, 1);
    assert_eq!(summary.rejected_games, 1);
    assert_eq!(summary.accepted_positions, 1);
    assert_eq!(summary.fifty_move_draws, 1);
    assert_eq!(summary.rejected_maximum_plies, 1);
}
