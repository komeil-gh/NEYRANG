use std::{
    fs,
    path::PathBuf,
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

use neyrang_nnue_reference::{HIDDEN_SIZE, INPUT_FEATURES, Network, NetworkParameters};

const FLOAT_COUNT: usize = INPUT_FEATURES * HIDDEN_SIZE + HIDDEN_SIZE + 2 * HIDDEN_SIZE + 1;

#[test]
fn verify_parity_emits_machine_readable_evidence_and_enforces_thresholds() {
    let directory = unique_temp_directory();
    fs::create_dir(&directory).unwrap();
    let raw_path = directory.join("raw.bin");
    let network_path = directory.join("network.nnue");
    let mismatched_path = directory.join("mismatched.nnue");
    let fens_path = directory.join("suite.fen");

    fs::write(&raw_path, raw_fixture()).unwrap();
    fs::write(&network_path, quantized_fixture(1)).unwrap();
    fs::write(&mismatched_path, quantized_fixture(0)).unwrap();
    fs::write(
        &fens_path,
        concat!(
            "4k3/8/8/8/3Q4/8/8/4K3 w - - 0 1\n",
            "4k3/8/8/8/3Q4/8/8/4K3 b - - 0 1\n",
        ),
    )
    .unwrap();

    let success = Command::new(env!("CARGO_BIN_EXE_verify-parity"))
        .arg(&raw_path)
        .arg(&network_path)
        .arg(&fens_path)
        .arg("8")
        .arg("2")
        .arg("--summary-only")
        .output()
        .unwrap();
    assert!(success.status.success());
    let stdout = String::from_utf8(success.stdout).unwrap();
    assert!(stdout.contains("\"schema\":\"neyrang-nnue-fen-parity-v1\""));
    assert!(stdout.contains("\"positions\":2"));
    assert!(stdout.contains("\"minimum_accumulator\":255"));
    assert!(stdout.contains("\"passed\":true"));
    assert!(!stdout.contains("\"samples\""));

    let failure = Command::new(env!("CARGO_BIN_EXE_verify-parity"))
        .arg(&raw_path)
        .arg(&mismatched_path)
        .arg(&fens_path)
        .arg("8")
        .arg("2")
        .output()
        .unwrap();
    assert!(!failure.status.success());
    let stdout = String::from_utf8(failure.stdout).unwrap();
    assert!(stdout.contains("\"passed\":false"));
    assert!(
        String::from_utf8(failure.stderr)
            .unwrap()
            .contains("parity thresholds exceeded")
    );

    fs::remove_dir_all(directory).unwrap();
}

fn raw_fixture() -> Vec<u8> {
    let mut values = vec![0.0_f32; FLOAT_COUNT];
    let feature_bias_offset = INPUT_FEATURES * HIDDEN_SIZE;
    let output_weight_offset = feature_bias_offset + HIDDEN_SIZE;
    values[feature_bias_offset..output_weight_offset].fill(1.0);
    values[output_weight_offset..output_weight_offset + 2 * HIDDEN_SIZE].fill(1.0 / 64.0);
    values
        .iter()
        .flat_map(|value| value.to_le_bytes())
        .collect()
}

fn quantized_fixture(output_weight: i16) -> Vec<u8> {
    Network::new(
        NetworkParameters {
            activation_quant: 255,
            output_quant: 64,
            centipawn_scale: 400,
        },
        vec![0; INPUT_FEATURES * HIDDEN_SIZE],
        vec![255; HIDDEN_SIZE],
        vec![output_weight; 2 * HIDDEN_SIZE],
        0,
    )
    .unwrap()
    .to_bytes()
}

fn unique_temp_directory() -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("neyrang-nnue-parity-{}-{nonce}", std::process::id()))
}
