use neyrang::uci::parser::{Command, parse};
use std::{
    io::{BufRead, BufReader, Write},
    process::{Command as ProcessCommand, Stdio},
    time::Duration,
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
    assert!(stdout.contains("id name NEYRANG 0.2.0"), "{stdout}");
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

#[test]
fn threaded_depth_search_emits_main_iterations_one_final_aggregate_and_one_bestmove() {
    let lines = run_uci_search(
        "setoption name Hash value 16\nsetoption name Threads value 4\nposition startpos\ngo depth 4\n",
        None,
    );
    let infos = lines
        .iter()
        .filter(|line| line.starts_with("info depth "))
        .collect::<Vec<_>>();
    let bestmoves = lines
        .iter()
        .filter(|line| line.starts_with("bestmove "))
        .collect::<Vec<_>>();

    assert_eq!(
        infos.len(),
        5,
        "four main iterations plus final aggregate: {lines:?}"
    );
    assert_eq!(bestmoves.len(), 1, "exactly one bestmove: {lines:?}");
    assert!(infos.last().expect("final info").contains(" depth 4 "));
    assert_legal_startpos_bestmove(bestmoves[0]);
}

#[test]
fn threaded_node_limit_is_global_and_stop_leaves_the_engine_ready() {
    let lines = run_uci_search(
        "setoption name Hash value 16\nsetoption name Threads value 4\nposition startpos\ngo nodes 10000\n",
        None,
    );
    let final_info = lines
        .iter()
        .rfind(|line| line.starts_with("info depth "))
        .expect("final aggregate info");

    assert_eq!(uci_field(final_info, "nodes"), Some(10_000));
    assert_eq!(
        lines
            .iter()
            .filter(|line| line.starts_with("bestmove "))
            .count(),
        1
    );
    assert!(lines.iter().any(|line| line == "readyok"));
}

#[test]
fn threaded_infinite_search_stops_cooperatively_and_emits_one_bestmove() {
    let lines = run_uci_search(
        "setoption name Hash value 16\nsetoption name Threads value 2\nposition startpos\ngo infinite\n",
        Some(Duration::from_millis(20)),
    );

    assert_eq!(
        lines
            .iter()
            .filter(|line| line.starts_with("bestmove "))
            .count(),
        1,
        "{lines:?}"
    );
    assert!(lines.iter().any(|line| line == "readyok"));
}

fn run_uci_search(commands: &str, stop_after: Option<Duration>) -> Vec<String> {
    let mut child = ProcessCommand::new(env!("CARGO_BIN_EXE_neyrang"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("NEYRANG binary should start");
    let mut stdin = child.stdin.take().expect("stdin is piped");
    let stdout = child.stdout.take().expect("stdout is piped");
    let mut reader = BufReader::new(stdout);
    stdin
        .write_all(format!("uci\n{commands}").as_bytes())
        .expect("commands should be written");
    stdin.flush().expect("commands should flush");
    if let Some(delay) = stop_after {
        std::thread::sleep(delay);
        stdin.write_all(b"stop\n").expect("stop should be written");
        stdin.flush().expect("stop should flush");
    }

    let mut lines = Vec::new();
    loop {
        let mut line = String::new();
        assert_ne!(reader.read_line(&mut line).expect("UCI output"), 0);
        let line = line.trim_end().to_owned();
        let done = line.starts_with("bestmove ");
        lines.push(line);
        if done {
            break;
        }
    }

    stdin
        .write_all(b"isready\n")
        .expect("isready should be written");
    stdin.flush().expect("isready should flush");
    loop {
        let mut line = String::new();
        assert_ne!(reader.read_line(&mut line).expect("UCI output"), 0);
        let line = line.trim_end().to_owned();
        let ready = line == "readyok";
        lines.push(line);
        if ready {
            break;
        }
    }
    stdin.write_all(b"quit\n").expect("quit should be written");
    drop(stdin);
    let status = child.wait().expect("NEYRANG should exit cleanly");
    assert!(status.success());
    lines
}

fn uci_field(line: &str, field: &str) -> Option<u64> {
    let tokens = line.split_whitespace().collect::<Vec<_>>();
    tokens
        .windows(2)
        .find(|pair| pair[0] == field)
        .and_then(|pair| pair[1].parse().ok())
}

fn assert_legal_startpos_bestmove(line: &str) {
    let bestmove = line.strip_prefix("bestmove ").expect("bestmove prefix");
    assert!(
        [
            "a2a3", "a2a4", "b2b3", "b2b4", "c2c3", "c2c4", "d2d3", "d2d4", "e2e3", "e2e4", "f2f3",
            "f2f4", "g2g3", "g2g4", "h2h3", "h2h4", "b1a3", "b1c3", "g1f3", "g1h3",
        ]
        .contains(&bestmove),
        "illegal start-position move: {bestmove}"
    );
}
