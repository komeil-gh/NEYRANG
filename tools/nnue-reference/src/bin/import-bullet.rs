use std::{
    env, fs,
    fs::OpenOptions,
    io::Write,
    path::{Path, PathBuf},
    process::ExitCode,
};

use neyrang_nnue_reference::{Network, NetworkParameters};

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("import-bullet: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let mut args = env::args_os().skip(1);
    let input = PathBuf::from(args.next().ok_or("missing Bullet quantised input path")?);
    let output = PathBuf::from(args.next().ok_or("missing NEYRANG output path")?);
    if args.next().is_some() {
        return Err("expected exactly two paths".to_string());
    }
    if !input.is_file() {
        return Err(format!("input is not a file: {}", input.display()));
    }
    if output.exists() {
        return Err(format!("refusing to overwrite: {}", output.display()));
    }

    let bytes = fs::read(&input).map_err(|error| format!("read input: {error}"))?;
    let network = Network::from_bullet_quantised(
        &bytes,
        NetworkParameters {
            activation_quant: 255,
            output_quant: 64,
            centipawn_scale: 400,
        },
    )
    .map_err(|error| format!("invalid Bullet tensor stream: {error}"))?;
    let artifact = network.to_bytes();
    write_exclusive(&output, &artifact)?;
    println!(
        "imported {} bytes into {}-byte NEYRANG NNUE artifact: {}",
        bytes.len(),
        artifact.len(),
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
