use std::{env, fs, process::ExitCode};

use neyrang::chess::Position;
use neyrang_nnue_data::{Game, GameResult, ScoredMove, encode_games};

fn main() -> ExitCode {
    let Some(output) = env::args_os().nth(1) else {
        eprintln!("usage: cargo run --example write-smoke -- OUTPUT.vf");
        return ExitCode::FAILURE;
    };
    match build_smoke().and_then(|games| encode_games(&games).map_err(|error| error.to_string())) {
        Ok(bytes) => match fs::write(&output, bytes) {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("write-smoke: {error}");
                ExitCode::FAILURE
            }
        },
        Err(error) => {
            eprintln!("write-smoke: {error}");
            ExitCode::FAILURE
        }
    }
}

fn build_smoke() -> Result<Vec<Game>, String> {
    Ok(vec![
        game(
            "r3k2r/8/8/8/8/8/8/R3K2R w KQkq - 7 42",
            &[("e1g1", 15), ("e8c8", -22)],
            GameResult::Draw,
        )?,
        game(
            "4k3/8/8/3pP3/8/8/8/4K3 w - d6 0 1",
            &[("e5d6", 87)],
            GameResult::WhiteWin,
        )?,
        game(
            "4k3/P7/8/8/8/8/8/4K3 w - - 0 1",
            &[("a7a8n", 311)],
            GameResult::BlackWin,
        )?,
    ])
}

fn game(fen: &str, line: &[(&str, i16)], result: GameResult) -> Result<Game, String> {
    let initial_position = Position::from_fen(fen).map_err(|error| error.to_string())?;
    let mut position = initial_position.clone();
    let mut moves = Vec::with_capacity(line.len());
    for &(notation, score_cp) in line {
        let mv = position
            .find_legal_move(notation)
            .ok_or_else(|| format!("illegal smoke move {notation} in {}", position.to_fen()))?;
        moves.push(ScoredMove { mv, score_cp });
        position.make_move(mv);
    }
    Ok(Game {
        initial_position,
        header_score: 0,
        result,
        extra: 0,
        moves,
    })
}
