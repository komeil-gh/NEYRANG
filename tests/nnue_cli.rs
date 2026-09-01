#![cfg(feature = "nnue")]

use std::{
    fs,
    path::PathBuf,
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

const HIDDEN_SIZE: usize = 128;
const INPUT_FEATURES: usize = 2 * 6 * 64;

#[test]
fn bench_nnue_reports_the_end_to_end_search_identity() {
    let directory = unique_temp_directory();
    fs::create_dir(&directory).unwrap();
    let network_path = directory.join("fixture.nnue");
    fs::write(&network_path, constant_network_artifact(32)).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_neyrang"))
        .arg("bench-nnue")
        .arg(&network_path)
        .arg("2")
        .output()
        .unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(stdout.contains("positions: 5"), "{stdout}");
    assert!(stdout.contains("nodes: "), "{stdout}");
    assert!(stdout.contains("nps: "), "{stdout}");
    assert!(stdout.contains("checksum: "), "{stdout}");

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
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("neyrang-nnue-cli-{}-{nonce}", std::process::id()))
}
