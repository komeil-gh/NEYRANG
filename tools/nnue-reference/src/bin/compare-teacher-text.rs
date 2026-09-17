use std::{
    env,
    fs::{self, File},
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
    process::ExitCode,
};

use neyrang::{
    chess::{Color, Position},
    sanj,
};
use neyrang_nnue_reference::{AccumulatorPair, Network, logistic_cp};

const HEADER: &str = "# neyrang-teacher-wdl-logit400-v2";
const EVAL_SCALE: f64 = 400.0;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("compare-teacher-text: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let mut args = env::args_os().skip(1);
    let baseline_path = PathBuf::from(args.next().ok_or("missing baseline network path")?);
    let candidate_path = PathBuf::from(args.next().ok_or("missing candidate network path")?);
    let corpus_path = PathBuf::from(args.next().ok_or("missing teacher text path")?);
    let baseline_mix = parse_mix(args.next().ok_or("missing baseline EvalMix")?)?;
    let candidate_mix = parse_mix(args.next().ok_or("missing candidate EvalMix")?)?;
    if args.next().is_some() {
        return Err(
            "expected two network paths, one teacher text path, and two EvalMix values".to_string(),
        );
    }

    let baseline = read_network(&baseline_path, "baseline")?;
    let candidate = read_network(&candidate_path, "candidate")?;
    if baseline.parameters().centipawn_scale != candidate.parameters().centipawn_scale {
        return Err("network centipawn scales differ".to_string());
    }

    let input = File::open(&corpus_path).map_err(|error| format!("open teacher text: {error}"))?;
    let mut lines = BufReader::new(input).lines();
    let header = lines
        .next()
        .ok_or("teacher text is empty")?
        .map_err(|error| format!("read teacher text header: {error}"))?;
    if header != HEADER {
        return Err(format!("unexpected teacher text header: {header:?}"));
    }

    let mut samples = 0_u64;
    let mut baseline_squared_error = 0.0;
    let mut candidate_squared_error = 0.0;
    let mut baseline_cross_entropy = 0.0;
    let mut candidate_cross_entropy = 0.0;
    let mut baseline_mix_mse = [0.0_f64; 101];
    let mut candidate_mix_mse = [0.0_f64; 101];
    let mut baseline_mix_bce = [0.0_f64; 101];
    let mut candidate_mix_bce = [0.0_f64; 101];
    let mut improved = 0_u64;
    let mut tied = 0_u64;
    let mut regressed = 0_u64;
    for (index, line) in lines.enumerate() {
        let line_number = index + 2;
        let line = line.map_err(|error| format!("read line {line_number}: {error}"))?;
        if line.is_empty() {
            return Err(format!("blank row at line {line_number}"));
        }
        let (fen, score) = line
            .rsplit_once('|')
            .ok_or_else(|| format!("missing score separator at line {line_number}"))?;
        let position = Position::from_fen(fen.trim())
            .map_err(|error| format!("invalid FEN at line {line_number}: {error}"))?;
        let score_white = score
            .trim()
            .parse::<i32>()
            .map_err(|_| format!("invalid white-relative score at line {line_number}"))?;
        let score_stm = match position.side_to_move() {
            Color::White => score_white,
            Color::Black => -score_white,
        };
        let target = logistic_cp(score_stm, EVAL_SCALE);
        let baseline_score = baseline.evaluate(
            &AccumulatorPair::refresh(&position, &baseline),
            position.side_to_move(),
        );
        let candidate_score = candidate.evaluate(
            &AccumulatorPair::refresh(&position, &candidate),
            position.side_to_move(),
        );
        let baseline_probability = logistic_cp(baseline_score, EVAL_SCALE);
        let candidate_probability = logistic_cp(candidate_score, EVAL_SCALE);
        let baseline_error = (baseline_probability - target).powi(2);
        let candidate_error = (candidate_probability - target).powi(2);
        baseline_squared_error += baseline_error;
        candidate_squared_error += candidate_error;
        baseline_cross_entropy += cross_entropy(target, baseline_probability);
        candidate_cross_entropy += cross_entropy(target, candidate_probability);
        let classical_score = sanj::evaluate(&position);
        for percent in 0..=100 {
            let baseline_prediction = logistic_cp(
                blend(classical_score, baseline_score, percent as u8),
                EVAL_SCALE,
            );
            let candidate_prediction = logistic_cp(
                blend(classical_score, candidate_score, percent as u8),
                EVAL_SCALE,
            );
            baseline_mix_mse[percent] += (baseline_prediction - target).powi(2);
            candidate_mix_mse[percent] += (candidate_prediction - target).powi(2);
            baseline_mix_bce[percent] += cross_entropy(target, baseline_prediction);
            candidate_mix_bce[percent] += cross_entropy(target, candidate_prediction);
        }
        if candidate_error < baseline_error {
            improved += 1;
        } else if candidate_error > baseline_error {
            regressed += 1;
        } else {
            tied += 1;
        }
        samples += 1;
    }
    if samples == 0 {
        return Err("teacher text contains no samples".to_string());
    }

    let baseline_mse = baseline_squared_error / samples as f64;
    let candidate_mse = candidate_squared_error / samples as f64;
    let baseline_bce = baseline_cross_entropy / samples as f64;
    let candidate_bce = candidate_cross_entropy / samples as f64;
    let (baseline_best_mse_mix, baseline_best_mse) = minimum_mean(&baseline_mix_mse, samples);
    let (candidate_best_mse_mix, candidate_best_mse) = minimum_mean(&candidate_mix_mse, samples);
    let (baseline_best_bce_mix, baseline_best_bce) = minimum_mean(&baseline_mix_bce, samples);
    let (candidate_best_bce_mix, candidate_best_bce) = minimum_mean(&candidate_mix_bce, samples);
    println!(
        concat!(
            "{{\"schema\":\"neyrang-teacher-text-comparison-v1\",",
            "\"baseline_network\":{:?},\"candidate_network\":{:?},",
            "\"corpus\":{:?},\"samples\":{},",
            "\"baseline_mse\":{:.12},\"candidate_mse\":{:.12},",
            "\"candidate_minus_baseline_mse\":{:.12},",
            "\"baseline_bce\":{:.12},\"candidate_bce\":{:.12},",
            "\"candidate_minus_baseline_bce\":{:.12},",
            "\"baseline_best_mse_mix\":{},\"baseline_best_mse\":{:.12},",
            "\"candidate_best_mse_mix\":{},\"candidate_best_mse\":{:.12},",
            "\"baseline_best_bce_mix\":{},\"baseline_best_bce\":{:.12},",
            "\"candidate_best_bce_mix\":{},\"candidate_best_bce\":{:.12},",
            "\"baseline_selected_mix\":{},\"baseline_selected_mse\":{:.12},",
            "\"baseline_selected_bce\":{:.12},",
            "\"candidate_selected_mix\":{},\"candidate_selected_mse\":{:.12},",
            "\"candidate_selected_bce\":{:.12},",
            "\"improved_positions\":{},\"tied_positions\":{},",
            "\"regressed_positions\":{},\"eval_scale\":400.0}}"
        ),
        baseline_path.to_string_lossy(),
        candidate_path.to_string_lossy(),
        corpus_path.to_string_lossy(),
        samples,
        baseline_mse,
        candidate_mse,
        candidate_mse - baseline_mse,
        baseline_bce,
        candidate_bce,
        candidate_bce - baseline_bce,
        baseline_best_mse_mix,
        baseline_best_mse,
        candidate_best_mse_mix,
        candidate_best_mse,
        baseline_best_bce_mix,
        baseline_best_bce,
        candidate_best_bce_mix,
        candidate_best_bce,
        baseline_mix,
        baseline_mix_mse[usize::from(baseline_mix)] / samples as f64,
        baseline_mix_bce[usize::from(baseline_mix)] / samples as f64,
        candidate_mix,
        candidate_mix_mse[usize::from(candidate_mix)] / samples as f64,
        candidate_mix_bce[usize::from(candidate_mix)] / samples as f64,
        improved,
        tied,
        regressed,
    );
    Ok(())
}

fn parse_mix(value: std::ffi::OsString) -> Result<u8, String> {
    let value = value
        .into_string()
        .map_err(|_| "EvalMix is not UTF-8".to_string())?;
    let parsed = value
        .parse::<u8>()
        .map_err(|_| format!("EvalMix must be an integer from 0 through 100: {value:?}"))?;
    if parsed > 100 {
        return Err(format!(
            "EvalMix must be an integer from 0 through 100: {value:?}"
        ));
    }
    Ok(parsed)
}

fn cross_entropy(target: f64, prediction: f64) -> f64 {
    let prediction = prediction.clamp(f64::EPSILON, 1.0 - f64::EPSILON);
    -target * prediction.ln() - (1.0 - target) * (1.0 - prediction).ln()
}

fn blend(classical: i32, nnue: i32, nnue_percent: u8) -> i32 {
    let nnue_weight = i64::from(nnue_percent);
    ((i64::from(classical) * (100 - nnue_weight) + i64::from(nnue) * nnue_weight) / 100) as i32
}

fn minimum_mean(values: &[f64; 101], samples: u64) -> (usize, f64) {
    let (index, value) = values
        .iter()
        .copied()
        .enumerate()
        .min_by(|left, right| {
            left.1
                .total_cmp(&right.1)
                .then_with(|| left.0.cmp(&right.0))
        })
        .expect("mix grid is non-empty");
    (index, value / samples as f64)
}

fn read_network(path: &Path, label: &str) -> Result<Network, String> {
    let bytes = fs::read(path).map_err(|error| format!("read {label} network: {error}"))?;
    Network::from_bytes(&bytes).map_err(|error| format!("invalid {label} network: {error}"))
}
