use std::{env, fs, path::PathBuf, process::ExitCode};

use neyrang_nnue_data::decode_games;
use neyrang_nnue_reference::{CorpusCompositionReport, Network, ScoreFitReport, diagnose_network};

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("diagnose-corpus: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let mut args = env::args_os().skip(1);
    let network_path = PathBuf::from(args.next().ok_or("missing network path")?);
    let corpus_path = PathBuf::from(args.next().ok_or("missing corpus path")?);
    if args.next().is_some() {
        return Err("expected one network path and one corpus path".to_string());
    }

    let network_bytes =
        fs::read(&network_path).map_err(|error| format!("read network: {error}"))?;
    let network =
        Network::from_bytes(&network_bytes).map_err(|error| format!("invalid network: {error}"))?;
    let corpus_bytes = fs::read(&corpus_path).map_err(|error| format!("read corpus: {error}"))?;
    let games = decode_games(&corpus_bytes).map_err(|error| format!("invalid corpus: {error}"))?;
    let report = diagnose_network(&network, &games).map_err(|error| error.to_string())?;

    println!(
        concat!(
            "{{\"schema\":\"neyrang-nnue-search-diagnostics-v1\",",
            "\"network\":{:?},\"corpus\":{:?},",
            "\"games\":{},\"positions\":{},",
            "\"network_score_range_cp\":[{},{}],",
            "\"classical_score_range_cp\":[{},{}],",
            "\"teacher_score_range_cp\":[{},{}],",
            "\"network_vs_search\":{},",
            "\"classical_vs_search\":{},",
            "\"network_vs_classical\":{},",
            "\"score_probability_mse\":{:.12},",
            "\"result_probability_mse\":{:.12},",
            "\"blended_probability_mse\":{:.12},",
            "\"mean_network_probability\":{:.12},",
            "\"mean_score_probability\":{:.12},",
            "\"mean_result_probability\":{:.12},",
            "\"composition\":{},",
            "\"eval_scale\":400.0,\"wdl_proportion\":0.75}}"
        ),
        network_path.to_string_lossy(),
        corpus_path.to_string_lossy(),
        report.games,
        report.positions,
        report.network_score_range_cp.0,
        report.network_score_range_cp.1,
        report.classical_score_range_cp.0,
        report.classical_score_range_cp.1,
        report.teacher_score_range_cp.0,
        report.teacher_score_range_cp.1,
        score_fit_json(&report.network_vs_search),
        score_fit_json(&report.classical_vs_search),
        score_fit_json(&report.network_vs_classical),
        report.score_probability_mse,
        report.result_probability_mse,
        report.blended_probability_mse,
        report.mean_network_probability,
        report.mean_score_probability,
        report.mean_result_probability,
        composition_json(&report.composition),
    );
    Ok(())
}

fn score_fit_json(report: &ScoreFitReport) -> String {
    format!(
        concat!(
            "{{\"samples\":{},",
            "\"reference_mean_cp\":{:.12},",
            "\"prediction_mean_cp\":{:.12},",
            "\"mean_error_cp\":{:.12},",
            "\"mean_absolute_error_cp\":{:.12},",
            "\"root_mean_square_error_cp\":{:.12},",
            "\"pearson_correlation\":{},",
            "\"sign_samples\":{},\"sign_agreements\":{},",
            "\"sign_agreement_rate\":{:.12}}}"
        ),
        report.samples,
        report.reference_mean_cp,
        report.prediction_mean_cp,
        report.mean_error_cp,
        report.mean_absolute_error_cp,
        report.root_mean_square_error_cp,
        optional_f64_json(report.pearson_correlation),
        report.sign_samples,
        report.sign_agreements,
        report.sign_agreement_rate,
    )
}

fn composition_json(report: &CorpusCompositionReport) -> String {
    format!(
        concat!(
            "{{\"early_ply\":{},\"low_piece_count\":{},",
            "\"high_eval_default\":{},\"high_eval_advanced\":{},",
            "\"tactical_move\":{},\"in_check\":{},",
            "\"castling_move\":{},",
            "\"result_inconsistent_over_2500_cp\":{},",
            "\"bullet_default_rejected\":{},",
            "\"advanced_deterministic_rejected\":{}}}"
        ),
        report.early_ply,
        report.low_piece_count,
        report.high_eval_default,
        report.high_eval_advanced,
        report.tactical_move,
        report.in_check,
        report.castling_move,
        report.result_inconsistent_over_2500_cp,
        report.bullet_default_rejected,
        report.advanced_deterministic_rejected,
    )
}

fn optional_f64_json(value: Option<f64>) -> String {
    value.map_or_else(|| "null".to_string(), |value| format!("{value:.12}"))
}
