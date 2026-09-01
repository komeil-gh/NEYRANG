use std::{
    env,
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    process::ExitCode,
};

use bullet_trainer::optimiser::utils::load_weights_from_file;
use neyrang_nnue_trainer::merge_factorised_raw_tensors;

const BUCKETS: usize = 3;
const HIDDEN_SIZE: usize = 128;
const BASE_FEATURES: usize = 768;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("export-factorised-raw: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let mut args = env::args_os().skip(1);
    let input = PathBuf::from(args.next().ok_or("missing checkpoint weights path")?);
    let output = PathBuf::from(args.next().ok_or("missing merged raw output path")?);
    if args.next().is_some() {
        return Err("too many arguments".to_string());
    }
    if !input.is_file() {
        return Err(format!(
            "checkpoint weights are not a file: {}",
            input.display()
        ));
    }
    if output.exists() {
        return Err(format!("refusing to overwrite: {}", output.display()));
    }

    let tensors = load_weights_from_file(
        input
            .to_str()
            .ok_or("checkpoint weights path is not UTF-8")?,
    );
    let merged = merge_factorised_raw_tensors(&tensors, BUCKETS, HIDDEN_SIZE, BASE_FEATURES)
        .map_err(|error| error.to_string())?;
    let bytes: Vec<u8> = merged
        .iter()
        .flat_map(|value| value.to_le_bytes())
        .collect();
    write_exclusive(&output, &bytes)?;
    println!(
        "exported {} checkpoint tensors into {} merged float values ({} bytes): {}",
        tensors.len(),
        merged.len(),
        bytes.len(),
        output.display()
    );
    Ok(())
}

fn write_exclusive(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| format!("create output: {error}"))?;
    if let Err(error) = output.write_all(bytes).and_then(|()| output.sync_all()) {
        drop(output);
        let _ = fs::remove_file(path);
        return Err(format!("write output: {error}"));
    }
    Ok(())
}
