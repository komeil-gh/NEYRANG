use std::{env, fs, path::PathBuf, process::ExitCode};

use neyrang::sanj::nnue::Network as EngineNetwork;
use neyrang_nnue_reference::{AccumulatorPair, Network as ReferenceNetwork, parse_fen_suite};

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("verify-engine-parity: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let mut args = env::args_os().skip(1);
    let network_path = PathBuf::from(args.next().ok_or("missing NEYRANG network path")?);
    let suite_path = PathBuf::from(args.next().ok_or("missing FEN suite path")?);
    if args.next().is_some() {
        return Err("too many arguments".to_owned());
    }

    let bytes = fs::read(&network_path).map_err(|error| format!("read network: {error}"))?;
    let reference = ReferenceNetwork::from_bytes(&bytes)
        .map_err(|error| format!("invalid reference network: {error}"))?;
    let engine = EngineNetwork::from_bytes(&bytes)
        .map_err(|error| format!("invalid engine network: {error}"))?;
    let suite_contents =
        fs::read_to_string(&suite_path).map_err(|error| format!("read FEN suite: {error}"))?;
    let fixtures =
        parse_fen_suite(&suite_contents).map_err(|error| format!("invalid FEN suite: {error}"))?;

    let mut mismatches = 0_usize;
    let mut maximum_absolute_delta = 0_i64;
    for fixture in &fixtures {
        let expected = reference.evaluate(
            &AccumulatorPair::refresh(&fixture.position, &reference),
            fixture.position.side_to_move(),
        );
        let actual = engine.evaluate(&fixture.position);
        if expected != actual {
            mismatches += 1;
            maximum_absolute_delta =
                maximum_absolute_delta.max(i64::from(expected).abs_diff(i64::from(actual)) as i64);
        }
    }
    let passed = mismatches == 0;

    println!(
        concat!(
            "{{\"schema\":\"neyrang-nnue-engine-parity-v1\",",
            "\"positions\":{},\"mismatches\":{},",
            "\"maximum_absolute_delta_cp\":{},\"passed\":{}}}"
        ),
        fixtures.len(),
        mismatches,
        maximum_absolute_delta,
        passed,
    );

    if !passed {
        return Err("engine scalar inference differs from the reference oracle".to_owned());
    }
    Ok(())
}
