use std::{
    env, fs,
    fs::OpenOptions,
    io::Write,
    path::{Path, PathBuf},
    process::ExitCode,
};

use neyrang_nnue_reference::{FeatureSet, FloatNetwork, NetworkParameters};

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("quantize-raw: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let mut args = env::args_os().skip(1);
    let input = PathBuf::from(args.next().ok_or("missing Bullet raw input path")?);
    let output = PathBuf::from(args.next().ok_or("missing NEYRANG output path")?);
    let activation_quant = parse_u16(
        "activation quantization",
        args.next().ok_or("missing activation quantization")?,
    )?;
    let output_quant = parse_u16(
        "output quantization",
        args.next().ok_or("missing output quantization")?,
    )?;
    let centipawn_scale = parse_i32(
        "centipawn scale",
        args.next().ok_or("missing centipawn scale")?,
    )?;
    let feature_set = match args.next() {
        None => FeatureSet::Chess768,
        Some(flag) if flag == "--feature-set" => {
            parse_feature_set(args.next().ok_or("missing value after --feature-set")?)?
        }
        Some(value) => return Err(format!("unknown argument: {value:?}")),
    };
    if args.next().is_some() {
        return Err("too many arguments".to_string());
    }
    if !input.is_file() {
        return Err(format!("input is not a file: {}", input.display()));
    }
    if output.exists() {
        return Err(format!("refusing to overwrite: {}", output.display()));
    }

    let bytes = fs::read(&input).map_err(|error| format!("read input: {error}"))?;
    let network =
        FloatNetwork::from_bullet_raw_with_feature_set(&bytes, centipawn_scale as f32, feature_set)
            .map_err(|error| format!("invalid Bullet raw tensor stream: {error}"))?
            .quantize(NetworkParameters {
                activation_quant,
                output_quant,
                centipawn_scale,
            })
            .map_err(|error| format!("quantization failed: {error}"))?;
    let artifact = network.to_bytes();
    write_exclusive(&output, &artifact)?;
    println!(
        "quantized {} bytes into {}-byte NEYRANG NNUE artifact: {}",
        bytes.len(),
        artifact.len(),
        output.display()
    );
    Ok(())
}

fn parse_feature_set(value: std::ffi::OsString) -> Result<FeatureSet, String> {
    match value.to_str() {
        Some("chess768") => Ok(FeatureSet::Chess768),
        Some("chess768x3hm") => Ok(FeatureSet::Chess768KingBucketsMirrored3),
        Some("chess768x3hm4ph") => Ok(FeatureSet::Chess768KingBucketsMirrored3PhaseHeads4),
        Some("chess768x3hmli") => Ok(FeatureSet::Chess768KingBucketsMirrored3LatentImbalance),
        Some(value) => Err(format!("unknown feature set: {value}")),
        None => Err("feature set is not UTF-8".to_string()),
    }
}

fn parse_u16(name: &str, value: std::ffi::OsString) -> Result<u16, String> {
    let value = value
        .into_string()
        .map_err(|_| format!("{name} is not UTF-8"))?;
    value
        .parse()
        .map_err(|_| format!("{name} is not an unsigned 16-bit integer: {value:?}"))
}

fn parse_i32(name: &str, value: std::ffi::OsString) -> Result<i32, String> {
    let value = value
        .into_string()
        .map_err(|_| format!("{name} is not UTF-8"))?;
    value
        .parse()
        .map_err(|_| format!("{name} is not a signed 32-bit integer: {value:?}"))
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
