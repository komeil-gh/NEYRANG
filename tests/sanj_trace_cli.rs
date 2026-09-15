#![cfg(feature = "sanj-tools")]

use std::{
    io::Write,
    process::{Command, Stdio},
};

#[test]
fn sanj_trace_cli_streams_stdin_without_touching_uci() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_neyrang"))
        .args(["sanj-trace", "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("feature-enabled NEYRANG binary starts");
    child
        .stdin
        .take()
        .expect("stdin is piped")
        .write_all(b"start\t0.5\trnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1\n")
        .expect("fixture is written");

    let output = child.wait_with_output().expect("trace process exits");
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let stdout = String::from_utf8(output.stdout).expect("trace output is UTF-8");
    let lines: Vec<_> = stdout.lines().collect();
    assert_eq!(lines.len(), 2);
    assert!(lines[0].starts_with("schema\trecord_id\ttarget\tfen\tstm\tphase\t"));
    assert!(lines[1].starts_with("neyrang-sanj-trace-v2\tstart\t0.5\t"));
    assert_eq!(lines[0].split('\t').count(), lines[1].split('\t').count());
}
