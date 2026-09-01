use std::{env, fs, path::PathBuf, process::ExitCode};

use neyrang_nnue_data::decode_games;
use neyrang_nnue_reference::{Network, compare_networks};

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("compare-corpus: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let mut args = env::args_os().skip(1);
    let baseline_path = PathBuf::from(args.next().ok_or("missing baseline network path")?);
    let candidate_path = PathBuf::from(args.next().ok_or("missing candidate network path")?);
    let corpus_path = PathBuf::from(args.next().ok_or("missing validation corpus path")?);
    let bootstrap_seed = parse_u64(
        "bootstrap seed",
        args.next().ok_or("missing bootstrap seed")?,
    )?;
    let bootstrap_replicates = parse_usize(
        "bootstrap replicate count",
        args.next().ok_or("missing bootstrap replicate count")?,
    )?;
    if args.next().is_some() {
        return Err("expected three paths, one seed and one replicate count".to_string());
    }

    let baseline = read_network(&baseline_path, "baseline")?;
    let candidate = read_network(&candidate_path, "candidate")?;
    let corpus_bytes =
        fs::read(&corpus_path).map_err(|error| format!("read validation corpus: {error}"))?;
    let games = decode_games(&corpus_bytes)
        .map_err(|error| format!("invalid validation corpus: {error}"))?;
    let report = compare_networks(
        &baseline,
        &candidate,
        &games,
        bootstrap_seed,
        bootstrap_replicates,
    )
    .map_err(|error| error.to_string())?;

    println!(
        concat!(
            "{{\"schema\":\"neyrang-nnue-paired-diagnostics-v1\",",
            "\"baseline_network\":{:?},\"candidate_network\":{:?},",
            "\"corpus\":{:?},\"games\":{},\"positions\":{},",
            "\"baseline_mse\":{:.12},\"candidate_mse\":{:.12},",
            "\"position_weighted_delta_mse\":{:.12},",
            "\"game_equal_mean_delta_mse\":{:.12},",
            "\"improved_games\":{},\"tied_games\":{},\"regressed_games\":{},",
            "\"bootstrap_lower_2_5_percentile\":{:.12},",
            "\"bootstrap_upper_97_5_percentile\":{:.12},",
            "\"bootstrap_seed\":{},\"bootstrap_replicates\":{},",
            "\"eval_scale\":400.0,\"wdl_proportion\":0.75}}"
        ),
        baseline_path.to_string_lossy(),
        candidate_path.to_string_lossy(),
        corpus_path.to_string_lossy(),
        report.games,
        report.positions,
        report.baseline_mse,
        report.candidate_mse,
        report.position_weighted_delta_mse,
        report.game_equal_mean_delta_mse,
        report.improved_games,
        report.tied_games,
        report.regressed_games,
        report.bootstrap_lower_2_5_percentile,
        report.bootstrap_upper_97_5_percentile,
        report.bootstrap_seed,
        report.bootstrap_replicates,
    );
    Ok(())
}

fn read_network(path: &PathBuf, label: &str) -> Result<Network, String> {
    let bytes = fs::read(path).map_err(|error| format!("read {label} network: {error}"))?;
    Network::from_bytes(&bytes).map_err(|error| format!("invalid {label} network: {error}"))
}

fn parse_u64(name: &str, value: std::ffi::OsString) -> Result<u64, String> {
    let value = value
        .into_string()
        .map_err(|_| format!("{name} is not UTF-8"))?;
    value
        .parse()
        .map_err(|_| format!("{name} is not an unsigned 64-bit integer: {value:?}"))
}

fn parse_usize(name: &str, value: std::ffi::OsString) -> Result<usize, String> {
    let value = value
        .into_string()
        .map_err(|_| format!("{name} is not UTF-8"))?;
    value
        .parse()
        .map_err(|_| format!("{name} is not a positive integer: {value:?}"))
}
