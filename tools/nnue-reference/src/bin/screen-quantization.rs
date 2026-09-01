use std::{env, fs, path::PathBuf, process::ExitCode};

use neyrang_nnue_reference::{
    FeatureSet, FloatNetwork, NetworkParameters, evaluate_parity, parse_fen_suite,
};

const CENTIPAWN_SCALE: i32 = 400;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("screen-quantization: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let mut args = env::args_os().skip(1);
    let raw_path = PathBuf::from(args.next().ok_or("missing Bullet raw network path")?);
    let suite_path = PathBuf::from(args.next().ok_or("missing FEN suite path")?);
    let activation_quant = parse_positive_u16(
        "activation quantization",
        args.next().ok_or("missing activation quantization")?,
    )?;
    let maximum_error_cp = parse_threshold(
        "maximum error",
        args.next().ok_or("missing maximum-error-cp threshold")?,
    )?;
    let mean_error_cp = parse_threshold(
        "mean error",
        args.next().ok_or("missing mean-error-cp threshold")?,
    )?;
    let mut feature_set = FeatureSet::Chess768;
    let mut feature_set_seen = false;
    let mut output_quants = Vec::new();
    while let Some(value) = args.next() {
        if value == "--feature-set" {
            if feature_set_seen {
                return Err("--feature-set may be specified only once".to_string());
            }
            feature_set =
                parse_feature_set(args.next().ok_or("missing value after --feature-set")?)?;
            feature_set_seen = true;
        } else {
            output_quants.push(parse_positive_u16("output quantization", value)?);
        }
    }
    if output_quants.is_empty() {
        return Err("at least one output quantization is required".to_string());
    }

    let raw_bytes = fs::read(&raw_path).map_err(|error| format!("read raw network: {error}"))?;
    let float_network = FloatNetwork::from_bullet_raw_with_feature_set(
        &raw_bytes,
        CENTIPAWN_SCALE as f32,
        feature_set,
    )
    .map_err(|error| format!("invalid raw network: {error}"))?;
    let suite_contents =
        fs::read_to_string(&suite_path).map_err(|error| format!("read FEN suite: {error}"))?;
    let positions: Vec<_> = parse_fen_suite(&suite_contents)
        .map_err(|error| format!("invalid FEN suite: {error}"))?
        .into_iter()
        .map(|item| item.position)
        .collect();

    let mut results = Vec::with_capacity(output_quants.len());
    for output_quant in output_quants {
        let network = float_network
            .quantize(NetworkParameters {
                activation_quant,
                output_quant,
                centipawn_scale: CENTIPAWN_SCALE,
            })
            .map_err(|error| format!("QA={activation_quant} QB={output_quant}: {error}"))?;
        let report = evaluate_parity(&float_network, &network, &positions)
            .map_err(|error| error.to_string())?;
        let passed = report.passes(maximum_error_cp, mean_error_cp);
        results.push((output_quant, report, passed));
    }

    print!(
        concat!(
            "{{\"schema\":\"neyrang-nnue-quantization-screen-v1\",",
            "\"positions\":{},\"activation_quant\":{},",
            "\"maximum_error_threshold_cp\":{:.12},",
            "\"mean_error_threshold_cp\":{:.12},\"candidates\":["
        ),
        positions.len(),
        activation_quant,
        maximum_error_cp,
        mean_error_cp,
    );
    for (index, (output_quant, report, passed)) in results.iter().enumerate() {
        if index > 0 {
            print!(",");
        }
        print!(
            concat!(
                "{{\"output_quant\":{},\"max_abs_error_cp\":{:.12},",
                "\"mean_abs_error_cp\":{:.12},",
                "\"minimum_accumulator\":{},\"maximum_accumulator\":{},",
                "\"passed\":{}}}"
            ),
            output_quant,
            report.max_abs_error_cp,
            report.mean_abs_error_cp,
            report.minimum_accumulator,
            report.maximum_accumulator,
            passed,
        );
    }
    println!("]}}");

    if !results.iter().any(|(_, _, passed)| *passed) {
        return Err("no quantization candidate passed thresholds".to_string());
    }
    Ok(())
}

fn parse_feature_set(value: std::ffi::OsString) -> Result<FeatureSet, String> {
    match value.to_str() {
        Some("chess768") => Ok(FeatureSet::Chess768),
        Some("chess768x3hm") => Ok(FeatureSet::Chess768KingBucketsMirrored3),
        Some(value) => Err(format!("unknown feature set: {value}")),
        None => Err("feature set is not UTF-8".to_string()),
    }
}

fn parse_positive_u16(name: &str, value: std::ffi::OsString) -> Result<u16, String> {
    let value = value
        .into_string()
        .map_err(|_| format!("{name} is not UTF-8"))?;
    let parsed: u16 = value
        .parse()
        .map_err(|_| format!("{name} is not an unsigned 16-bit integer: {value:?}"))?;
    if parsed == 0 {
        return Err(format!("{name} must be positive"));
    }
    Ok(parsed)
}

fn parse_threshold(name: &str, value: std::ffi::OsString) -> Result<f64, String> {
    let value = value
        .into_string()
        .map_err(|_| format!("{name} threshold is not UTF-8"))?;
    let parsed: f64 = value
        .parse()
        .map_err(|_| format!("{name} threshold is not a number: {value:?}"))?;
    if !parsed.is_finite() || parsed < 0.0 {
        return Err(format!("{name} threshold must be finite and non-negative"));
    }
    Ok(parsed)
}
