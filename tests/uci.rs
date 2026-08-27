use neyrang::uci::parser::{Command, parse};
use std::{
    io::Write,
    process::{Command as ProcessCommand, Stdio},
};

#[test]
fn parser_preserves_fen_and_following_moves() {
    let command = parse("position fen r3k2r/8/8/8/8/8/8/R3K2R w KQkq - 0 1 moves e1g1 e8c8")
        .expect("position command should parse");

    let Command::Position(specification) = command else {
        panic!("expected a position command");
    };
    assert_eq!(
        specification.fen.as_deref(),
        Some("r3k2r/8/8/8/8/8/8/R3K2R w KQkq - 0 1")
    );
    assert_eq!(specification.moves, ["e1g1", "e8c8"]);
}

#[test]
fn parser_collects_all_supported_go_limits() {
    let command =
        parse("go wtime 60000 btime 59000 winc 500 binc 400 movestogo 20 depth 12 nodes 123456")
            .expect("go command should parse");

    let Command::Go(parameters) = command else {
        panic!("expected a go command");
    };
    assert_eq!(parameters.wtime_ms, Some(60_000));
    assert_eq!(parameters.btime_ms, Some(59_000));
    assert_eq!(parameters.winc_ms, Some(500));
    assert_eq!(parameters.binc_ms, Some(400));
    assert_eq!(parameters.moves_to_go, Some(20));
    assert_eq!(parameters.depth, Some(12));
    assert_eq!(parameters.nodes, Some(123_456));
}

#[test]
fn uci_process_reports_identity_readiness_and_a_legal_bestmove() {
    let mut child = ProcessCommand::new(env!("CARGO_BIN_EXE_neyrang"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("NEYRANG binary should start");
    child
        .stdin
        .take()
        .expect("stdin is piped")
        .write_all(b"uci\nisready\nposition startpos\ngo depth 2\nquit\n")
        .expect("commands should be written");
    let output = child.wait_with_output().expect("NEYRANG should exit cleanly");
    let stdout = String::from_utf8(output.stdout).expect("UCI output is UTF-8");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(stdout.contains("id name NEYRANG 0.1.0"), "{stdout}");
    assert!(stdout.contains("uciok"), "{stdout}");
    assert!(stdout.contains("readyok"), "{stdout}");
    let bestmove = stdout
        .lines()
        .find_map(|line| line.strip_prefix("bestmove "))
        .expect("bestmove line should be present");
    assert!(
        [
            "a2a3", "a2a4", "b2b3", "b2b4", "c2c3", "c2c4", "d2d3", "d2d4", "e2e3", "e2e4", "f2f3",
            "f2f4", "g2g3", "g2g4", "h2h3", "h2h4", "b1a3", "b1c3", "g1f3", "g1h3"
        ]
        .contains(&bestmove),
        "illegal start-position move: {bestmove}"
    );
}
