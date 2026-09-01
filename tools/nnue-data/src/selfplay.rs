use std::sync::atomic::AtomicBool;

use neyrang::{
    chess::{Color, Move, Position},
    rekhne::{SearchLimits, Searcher},
};

use crate::{Game, GameResult, ScoredMove};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SearchSample {
    pub best_move: Move,
    pub score_cp: i32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompletionReason {
    Checkmate,
    Stalemate,
    FiftyMoveRule,
    ThreefoldRepetition,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RejectionReason {
    TerminalOpening,
    MaximumPlies,
    MissingSearchMove,
    IllegalSearchMove,
    SearchMutatedPosition,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcceptedGame {
    pub game: Game,
    pub completion: CompletionReason,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GameAttempt {
    Accepted(Box<AcceptedGame>),
    Rejected(RejectionReason),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SelfPlayConfig {
    pub nodes_per_move: u64,
    pub hash_megabytes: usize,
    pub maximum_plies: usize,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SelfPlaySummary {
    pub attempted_games: u64,
    pub accepted_games: u64,
    pub rejected_games: u64,
    pub accepted_positions: u64,
    pub checkmates: u64,
    pub stalemates: u64,
    pub fifty_move_draws: u64,
    pub threefold_draws: u64,
    pub rejected_terminal_openings: u64,
    pub rejected_maximum_plies: u64,
    pub rejected_missing_search_move: u64,
    pub rejected_illegal_search_move: u64,
    pub rejected_search_mutated_position: u64,
}

impl SelfPlaySummary {
    pub fn record(&mut self, attempt: &GameAttempt) {
        self.attempted_games += 1;
        match attempt {
            GameAttempt::Accepted(accepted) => {
                self.accepted_games += 1;
                self.accepted_positions += accepted.game.moves.len() as u64;
                match accepted.completion {
                    CompletionReason::Checkmate => self.checkmates += 1,
                    CompletionReason::Stalemate => self.stalemates += 1,
                    CompletionReason::FiftyMoveRule => self.fifty_move_draws += 1,
                    CompletionReason::ThreefoldRepetition => self.threefold_draws += 1,
                }
            }
            GameAttempt::Rejected(reason) => {
                self.rejected_games += 1;
                match reason {
                    RejectionReason::TerminalOpening => self.rejected_terminal_openings += 1,
                    RejectionReason::MaximumPlies => self.rejected_maximum_plies += 1,
                    RejectionReason::MissingSearchMove => {
                        self.rejected_missing_search_move += 1;
                    }
                    RejectionReason::IllegalSearchMove => {
                        self.rejected_illegal_search_move += 1;
                    }
                    RejectionReason::SearchMutatedPosition => {
                        self.rejected_search_mutated_position += 1;
                    }
                }
            }
        }
    }
}

pub fn white_relative_score(side_to_move: Color, score_cp: i32) -> i16 {
    let score = i64::from(score_cp);
    let white_score = if side_to_move == Color::White {
        score
    } else {
        -score
    };
    white_score.clamp(i64::from(i16::MIN), i64::from(i16::MAX)) as i16
}

pub fn record_game(initial_position: Position, config: SelfPlayConfig) -> GameAttempt {
    let stop = AtomicBool::new(false);
    let mut searcher = Searcher::with_hash(&stop, config.hash_megabytes);
    let limits = SearchLimits::nodes(config.nodes_per_move);
    record_game_with(
        initial_position,
        config.maximum_plies,
        |position, hashes| {
            let result = searcher.search(position, &limits, hashes, |_| {});
            result.best_move.map(|best_move| SearchSample {
                best_move,
                score_cp: result.score,
            })
        },
    )
}

pub fn record_game_with<F>(
    initial_position: Position,
    maximum_plies: usize,
    mut search: F,
) -> GameAttempt
where
    F: FnMut(&mut Position, &[u64]) -> Option<SearchSample>,
{
    let mut position = initial_position.clone();
    let mut hashes = vec![position.repetition_hash()];
    if completed_game(&mut position, &hashes).is_some() {
        return GameAttempt::Rejected(RejectionReason::TerminalOpening);
    }

    let mut moves = Vec::new();
    loop {
        if moves.len() == maximum_plies {
            return GameAttempt::Rejected(RejectionReason::MaximumPlies);
        }

        let side_to_move = position.side_to_move();
        let position_before_search = position.clone();
        let Some(sample) = search(&mut position, &hashes) else {
            return GameAttempt::Rejected(RejectionReason::MissingSearchMove);
        };
        if position != position_before_search {
            return GameAttempt::Rejected(RejectionReason::SearchMutatedPosition);
        }
        if !position
            .legal_moves()
            .iter()
            .any(|candidate| *candidate == sample.best_move)
        {
            return GameAttempt::Rejected(RejectionReason::IllegalSearchMove);
        }

        moves.push(ScoredMove {
            mv: sample.best_move,
            score_cp: white_relative_score(side_to_move, sample.score_cp),
        });
        position.make_move(sample.best_move);
        hashes.push(position.repetition_hash());

        if let Some((result, completion)) = completed_game(&mut position, &hashes) {
            return GameAttempt::Accepted(Box::new(AcceptedGame {
                game: Game {
                    initial_position,
                    header_score: 0,
                    result,
                    extra: 0,
                    moves,
                },
                completion,
            }));
        }
    }
}

fn completed_game(
    position: &mut Position,
    repetition_hashes: &[u64],
) -> Option<(GameResult, CompletionReason)> {
    if position.legal_moves().is_empty() {
        if position.is_in_check(position.side_to_move()) {
            let result = match position.side_to_move() {
                Color::White => GameResult::BlackWin,
                Color::Black => GameResult::WhiteWin,
            };
            return Some((result, CompletionReason::Checkmate));
        }
        return Some((GameResult::Draw, CompletionReason::Stalemate));
    }
    if position.halfmove_clock() >= 100 {
        return Some((GameResult::Draw, CompletionReason::FiftyMoveRule));
    }
    let current = repetition_hashes.last().copied()?;
    if repetition_hashes
        .iter()
        .rev()
        .take(position.halfmove_clock() as usize + 1)
        .filter(|&&hash| hash == current)
        .take(3)
        .count()
        >= 3
    {
        return Some((GameResult::Draw, CompletionReason::ThreefoldRepetition));
    }
    None
}
