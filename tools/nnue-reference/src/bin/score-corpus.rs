use std::{env, fs, path::PathBuf, process::ExitCode};

use neyrang_nnue_data::{GameResult, decode_games};
use neyrang_nnue_reference::{AccumulatorPair, Network, blended_target, logistic_cp};

const EVAL_SCALE: f64 = 400.0;
const WDL_PROPORTION: f64 = 0.75;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("score-corpus: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let mut args = env::args_os().skip(1);
    let network_path = PathBuf::from(args.next().ok_or("missing NEYRANG network path")?);
    let corpus_path = PathBuf::from(args.next().ok_or("missing corpus path")?);
    if args.next().is_some() {
        return Err("expected exactly two paths".to_string());
    }
    let network_bytes =
        fs::read(&network_path).map_err(|error| format!("read network: {error}"))?;
    let network = Network::from_bytes(&network_bytes)
        .map_err(|error| format!("invalid NEYRANG network: {error}"))?;
    let corpus_bytes = fs::read(&corpus_path).map_err(|error| format!("read corpus: {error}"))?;
    let games = decode_games(&corpus_bytes).map_err(|error| format!("invalid corpus: {error}"))?;

    let mut positions = 0_u64;
    let mut squared_error = 0.0_f64;
    let mut baseline_squared_error = 0.0_f64;
    let mut prediction_sum = 0.0_f64;
    let mut target_sum = 0.0_f64;
    for game in games {
        let result_white = match game.result {
            GameResult::BlackWin => 0.0,
            GameResult::Draw => 0.5,
            GameResult::WhiteWin => 1.0,
        };
        let mut position = game.initial_position;
        let mut accumulators = AccumulatorPair::refresh(&position, &network);
        for scored_move in game.moves {
            let side_to_move = position.side_to_move();
            let prediction = logistic_cp(network.evaluate(&accumulators, side_to_move), EVAL_SCALE);
            let target = blended_target(
                scored_move.score_cp,
                result_white,
                side_to_move,
                WDL_PROPORTION,
                EVAL_SCALE,
            );
            squared_error += (prediction - target).powi(2);
            baseline_squared_error += (0.5 - target).powi(2);
            prediction_sum += prediction;
            target_sum += target;
            positions += 1;

            let before = position.clone();
            position.make_move(scored_move.mv);
            accumulators.update(&before, &position, &network);
        }
    }
    if positions == 0 {
        return Err("corpus contains no scored positions".to_string());
    }
    let count = positions as f64;
    let mse = squared_error / count;
    println!(
        concat!(
            "{{\"schema\":\"neyrang-nnue-diagnostics-v1\",",
            "\"network\":\"{}\",\"corpus\":\"{}\",",
            "\"positions\":{},\"mse\":{:.12},\"rmse\":{:.12},",
            "\"even_baseline_mse\":{:.12},\"mean_prediction\":{:.12},",
            "\"mean_target\":{:.12},\"eval_scale\":{:.1},",
            "\"wdl_proportion\":{:.2}}}"
        ),
        network_path.display(),
        corpus_path.display(),
        positions,
        mse,
        mse.sqrt(),
        baseline_squared_error / count,
        prediction_sum / count,
        target_sum / count,
        EVAL_SCALE,
        WDL_PROPORTION,
    );
    Ok(())
}
