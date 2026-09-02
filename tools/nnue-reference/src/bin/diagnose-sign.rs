use std::{env, fs, path::PathBuf, process::ExitCode};

use neyrang_nnue_data::decode_games;
use neyrang_nnue_reference::{
    NamedSignSliceReport, Network, SignTransitionReport, diagnose_sign_disagreements,
};

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("diagnose-sign: {error}");
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
    let report = diagnose_sign_disagreements(
        &baseline,
        &candidate,
        &games,
        bootstrap_seed,
        bootstrap_replicates,
    )
    .map_err(|error| error.to_string())?;

    println!(
        concat!(
            "{{\"schema\":\"neyrang-nnue-sign-diagnostics-v1\",",
            "\"baseline_network\":{:?},\"candidate_network\":{:?},",
            "\"corpus\":{:?},\"games\":{},\"signed_games\":{},\"positions\":{},",
            "\"overall\":{},",
            "\"score_magnitude_bands\":{},",
            "\"stm_king_buckets\":{},\"ntm_king_buckets\":{},",
            "\"king_bucket_pairs\":{},\"piece_count_bands\":{},",
            "\"side_to_move\":{},\"contexts\":{},",
            "\"game_equal_mean_delta_sign_agreement\":{:.12},",
            "\"improved_games\":{},\"tied_games\":{},\"regressed_games\":{},",
            "\"bootstrap_lower_2_5_percentile\":{:.12},",
            "\"bootstrap_upper_97_5_percentile\":{:.12},",
            "\"bootstrap_seed\":{},\"bootstrap_replicates\":{}}}"
        ),
        baseline_path.to_string_lossy(),
        candidate_path.to_string_lossy(),
        corpus_path.to_string_lossy(),
        report.games,
        report.signed_games,
        report.positions,
        transition_json(&report.overall),
        slices_json(&report.score_magnitude_bands),
        slices_json(&report.stm_king_buckets),
        slices_json(&report.ntm_king_buckets),
        slices_json(&report.king_bucket_pairs),
        slices_json(&report.piece_count_bands),
        slices_json(&report.side_to_move),
        slices_json(&report.contexts),
        report.game_equal_mean_delta_sign_agreement,
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

fn transition_json(report: &SignTransitionReport) -> String {
    format!(
        concat!(
            "{{\"samples\":{},\"both_correct\":{},",
            "\"baseline_only_correct\":{},\"candidate_only_correct\":{},",
            "\"neither_correct\":{},\"baseline_agreements\":{},",
            "\"candidate_agreements\":{},",
            "\"baseline_agreement_rate\":{:.12},",
            "\"candidate_agreement_rate\":{:.12},",
            "\"candidate_minus_baseline_rate\":{:.12}}}"
        ),
        report.samples,
        report.both_correct,
        report.baseline_only_correct,
        report.candidate_only_correct,
        report.neither_correct,
        report.baseline_agreements,
        report.candidate_agreements,
        report.baseline_agreement_rate,
        report.candidate_agreement_rate,
        report.candidate_minus_baseline_rate,
    )
}

fn slices_json(slices: &[NamedSignSliceReport]) -> String {
    let entries = slices
        .iter()
        .map(|slice| {
            format!(
                "{{\"key\":{:?},\"transitions\":{}}}",
                slice.key,
                transition_json(&slice.transitions)
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!("[{entries}]")
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
