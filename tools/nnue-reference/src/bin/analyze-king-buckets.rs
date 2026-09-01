use std::{env, fs, path::PathBuf, process::ExitCode};

use neyrang_nnue_data::decode_games;
use neyrang_nnue_reference::analyze_king_buckets;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("analyze-king-buckets: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let mut args = env::args_os().skip(1);
    let corpus_path = PathBuf::from(args.next().ok_or("missing corpus path")?);
    if args.next().is_some() {
        return Err("expected exactly one corpus path".to_string());
    }

    let corpus_bytes = fs::read(&corpus_path).map_err(|error| format!("read corpus: {error}"))?;
    let games = decode_games(&corpus_bytes).map_err(|error| format!("invalid corpus: {error}"))?;
    let report = analyze_king_buckets(&games).map_err(|error| error.to_string())?;

    println!(
        concat!(
            "{{\"schema\":\"neyrang-nnue-king-bucket-analysis-v1\",",
            "\"corpus\":{:?},\"games\":{},\"positions\":{},",
            "\"perspective_samples\":{},",
            "\"oriented_king_squares\":{:?},",
            "\"horizontally_mirrored_king_squares\":{:?}}}"
        ),
        corpus_path.to_string_lossy(),
        report.games,
        report.positions,
        report.perspective_samples,
        report.oriented_king_squares,
        report.horizontally_mirrored_king_squares,
    );
    Ok(())
}
