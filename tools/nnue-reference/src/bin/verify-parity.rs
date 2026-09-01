use std::{env, fs, path::PathBuf, process::ExitCode};

use neyrang_nnue_reference::{FloatNetwork, Network, evaluate_parity, parse_fen_suite};

const CENTIPAWN_SCALE: f32 = 400.0;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("verify-parity: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let mut args = env::args_os().skip(1);
    let raw_path = PathBuf::from(args.next().ok_or("missing Bullet raw network path")?);
    let network_path = PathBuf::from(args.next().ok_or("missing NEYRANG network path")?);
    let suite_path = PathBuf::from(args.next().ok_or("missing FEN suite path")?);
    let maximum_error_cp = parse_threshold(
        "maximum error",
        args.next().ok_or("missing maximum-error-cp threshold")?,
    )?;
    let mean_error_cp = parse_threshold(
        "mean error",
        args.next().ok_or("missing mean-error-cp threshold")?,
    )?;
    let summary_only = match args.next() {
        None => false,
        Some(value) if value == "--summary-only" => true,
        Some(value) => return Err(format!("unknown argument: {value:?}")),
    };
    if args.next().is_some() {
        return Err("too many arguments".to_string());
    }

    let network_bytes =
        fs::read(&network_path).map_err(|error| format!("read NEYRANG network: {error}"))?;
    let quantized_network = Network::from_bytes(&network_bytes)
        .map_err(|error| format!("invalid NEYRANG network: {error}"))?;
    let raw_bytes = fs::read(&raw_path).map_err(|error| format!("read raw network: {error}"))?;
    let float_network = FloatNetwork::from_bullet_raw_with_feature_set(
        &raw_bytes,
        CENTIPAWN_SCALE,
        quantized_network.feature_set(),
    )
    .map_err(|error| format!("invalid raw network: {error}"))?;
    let suite_contents =
        fs::read_to_string(&suite_path).map_err(|error| format!("read FEN suite: {error}"))?;
    let frozen =
        parse_fen_suite(&suite_contents).map_err(|error| format!("invalid FEN suite: {error}"))?;
    let positions: Vec<_> = frozen.into_iter().map(|item| item.position).collect();
    let report = evaluate_parity(&float_network, &quantized_network, &positions)
        .map_err(|error| error.to_string())?;
    let passed = report.passes(maximum_error_cp, mean_error_cp);

    print!(
        concat!(
            "{{\"schema\":\"neyrang-nnue-fen-parity-v1\",",
            "\"positions\":{},\"max_abs_error_cp\":{:.12},",
            "\"mean_abs_error_cp\":{:.12},",
            "\"minimum_accumulator\":{},\"maximum_accumulator\":{},",
            "\"maximum_error_threshold_cp\":{:.12},",
            "\"mean_error_threshold_cp\":{:.12},\"passed\":{}"
        ),
        report.samples.len(),
        report.max_abs_error_cp,
        report.mean_abs_error_cp,
        report.minimum_accumulator,
        report.maximum_accumulator,
        maximum_error_cp,
        mean_error_cp,
        passed,
    );
    if summary_only {
        println!("}}");
    } else {
        print!(",\"samples\":[");
        for (index, sample) in report.samples.iter().enumerate() {
            if index > 0 {
                print!(",");
            }
            print!(
                "[{:.12},{},{:.12}]",
                sample.float_cp, sample.quantized_cp, sample.abs_error_cp
            );
        }
        println!("]}}");
    }

    if !passed {
        return Err("parity thresholds exceeded".to_string());
    }
    Ok(())
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
