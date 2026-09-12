#![cfg(feature = "nnue")]

use std::{
    fs,
    io::Write,
    path::PathBuf,
    process::{Command, Stdio},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

const HIDDEN_SIZE: usize = 128;
const INPUT_FEATURES: usize = 2 * 6 * 64;

#[test]
fn uci_eval_file_loads_fail_closed_and_activates_nnue_explicitly() {
    let directory = unique_temp_directory();
    fs::create_dir(&directory).unwrap();
    let network_path = directory.join("fixture.nnue");
    fs::write(&network_path, constant_network_artifact(32)).unwrap();

    let mut child = Command::new(env!("CARGO_BIN_EXE_neyrang"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("NEYRANG binary must start");
    writeln!(
        child.stdin.as_mut().expect("stdin is piped"),
        "uci\nsetoption name EvalFile value {}\nposition startpos\neval\nisready\nquit",
        network_path.display()
    )
    .unwrap();
    let output = child.wait_with_output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();

    assert!(output.status.success());
    assert!(
        stdout.contains("option name EvalFile type string default <empty>"),
        "{stdout}"
    );
    assert!(stdout.contains("info string eval 25 cp"), "{stdout}");
    assert!(stdout.contains("readyok"), "{stdout}");

    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn loaded_nnue_is_owned_safely_by_lazy_smp_workers() {
    let directory = unique_temp_directory();
    fs::create_dir(&directory).unwrap();
    let network_path = directory.join("fixture.nnue");
    fs::write(&network_path, constant_network_artifact(32)).unwrap();

    let mut child = Command::new(env!("CARGO_BIN_EXE_neyrang"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("NEYRANG binary must start");
    writeln!(
        child.stdin.as_mut().expect("stdin is piped"),
        "setoption name EvalFile value {}\nsetoption name Threads value 2\nposition startpos\ngo depth 3\nquit",
        network_path.display()
    )
    .unwrap();
    let output = child.wait_with_output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        stdout
            .lines()
            .filter(|line| line.starts_with("bestmove "))
            .count(),
        1,
        "{stdout}"
    );

    fs::remove_dir_all(directory).unwrap();
}

fn constant_network_artifact(output_bias: i32) -> Vec<u8> {
    let payload_size = (INPUT_FEATURES * HIDDEN_SIZE + HIDDEN_SIZE + 2 * HIDDEN_SIZE) * 2 + 4;
    let mut payload = vec![0; payload_size];
    payload[payload_size - 4..].copy_from_slice(&output_bias.to_le_bytes());

    let mut bytes = Vec::with_capacity(32 + payload_size);
    bytes.extend_from_slice(b"NEYRANG\0");
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&(HIDDEN_SIZE as u16).to_le_bytes());
    bytes.extend_from_slice(&32_u16.to_le_bytes());
    bytes.extend_from_slice(&16_u16.to_le_bytes());
    bytes.extend_from_slice(&0_u16.to_le_bytes());
    bytes.extend_from_slice(&400_i32.to_le_bytes());
    bytes.extend_from_slice(&(payload_size as u32).to_le_bytes());
    bytes.extend_from_slice(&crc32(&payload).to_le_bytes());
    bytes.extend_from_slice(&payload);
    bytes
}

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = u32::MAX;
    for &byte in bytes {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            let mask = 0_u32.wrapping_sub(crc & 1);
            crc = (crc >> 1) ^ (0xedb8_8320 & mask);
        }
    }
    !crc
}

fn unique_temp_directory() -> PathBuf {
    static NEXT_ID: AtomicU64 = AtomicU64::new(0);
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("neyrang-nnue-uci-{}-{nonce}-{id}", std::process::id()))
}
