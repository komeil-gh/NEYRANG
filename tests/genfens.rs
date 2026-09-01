use std::{
    collections::HashSet,
    process::{Command, Stdio},
};

use neyrang::{
    chess::{Color, Position},
    sanj,
    tools::genfens::{GenerationSummary, run},
};

fn generate(command: &str) -> (GenerationSummary, Vec<String>) {
    let mut output = Vec::new();
    let summary = run(command, &mut output).expect("genfens command should succeed");
    let text = String::from_utf8(output).expect("genfens output must be UTF-8");
    let lines = text.lines().map(str::to_owned).collect::<Vec<_>>();
    (summary, lines)
}

fn fens(lines: &[String]) -> Vec<&str> {
    lines
        .iter()
        .map(|line| {
            line.strip_prefix("info string genfens ")
                .expect("every line must use the exact OpenBench prefix")
        })
        .collect()
}

#[test]
fn deterministic_output_is_legal_balanced_and_diverse() {
    let command = "genfens 32 seed 2026090100000001 book None";
    let (first_summary, first) = generate(command);
    let (second_summary, second) = generate(command);

    assert_eq!(first_summary, second_summary);
    assert_eq!(first, second);
    assert_eq!(first_summary.generated, 32);
    assert_eq!(first.len(), 32);

    let openings = fens(&first);
    assert_eq!(openings.iter().copied().collect::<HashSet<_>>().len(), 32);
    for fen in openings {
        let mut position = Position::from_fen(fen).expect("generated FEN must parse");
        assert!(
            !position.legal_moves().is_empty(),
            "terminal opening: {fen}"
        );
        assert!(
            !position.is_in_check(position.side_to_move()),
            "opening leaves the side to move in check: {fen}"
        );
        assert_eq!(
            position.side_to_move(),
            Color::White,
            "odd-ply opening: {fen}"
        );
        assert!(
            sanj::evaluate(&position).abs() <= 180,
            "opening exceeds the registered balance bound: {fen}"
        );
    }
}

#[test]
fn every_seed_bit_affects_generation() {
    let (_, low) = generate("genfens 8 seed 7 book None");
    let (_, high) = generate("genfens 8 seed 4294967303 book None");

    assert_ne!(low, high, "upper seed bits must not be discarded");
}

#[test]
fn one_batch_matches_independently_sharded_seed_offsets() {
    let seed = 0x1234_5678_9abc_def0_u64;
    let (_, batch) = generate(&format!("genfens 4 seed {seed} book None"));
    let singles = (0..4)
        .flat_map(|offset| {
            let (_, output) = generate(&format!(
                "genfens 1 seed {} book None",
                seed.wrapping_add(offset)
            ));
            output
        })
        .collect::<Vec<_>>();

    assert_eq!(batch, singles);
}

#[test]
fn malformed_or_unsupported_commands_fail_closed_without_output() {
    for command in [
        "genfens 0 seed 1 book None",
        "genfens 1 seed -1 book None",
        "genfens 1 seed 18446744073709551616 book None",
        "genfens 1 seed 1 book openings.epd",
        "genfens 1 book None seed 1",
        "genfens 1 seed 1 book None surprise 2",
        "genfens 1 seed 1 book None minplies 3",
        "genfens 1 seed 1 book None maxplies 62",
        "genfens 1 seed 1 book None margin -1",
        "genfens 1 seed 1 book None maxeval 2001",
        "genfens 1 seed 1 book None margin",
    ] {
        let mut output = Vec::new();
        assert!(run(command, &mut output).is_err(), "accepted: {command}");
        assert!(output.is_empty(), "partial output for rejected command");
    }
}

#[test]
fn executable_accepts_the_exact_openbench_argv_shape() {
    let output = Command::new(env!("CARGO_BIN_EXE_neyrang"))
        .arg("genfens 4 seed 18446744073709551615 book None")
        .arg("quit")
        .stdin(Stdio::null())
        .output()
        .expect("NEYRANG binary should start");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty(), "successful genfens must be quiet");
    let stdout = String::from_utf8(output.stdout).expect("stdout must be UTF-8");
    let lines = stdout.lines().map(str::to_owned).collect::<Vec<_>>();
    assert_eq!(lines.len(), 4, "{stdout}");
    assert_eq!(fens(&lines).len(), 4);
}
