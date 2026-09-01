use std::{
    fs,
    path::PathBuf,
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

use neyrang_nnue_reference::{HIDDEN_SIZE, INPUT_FEATURES};

const FLOAT_COUNT: usize = INPUT_FEATURES * HIDDEN_SIZE + HIDDEN_SIZE + 2 * HIDDEN_SIZE + 1;

#[test]
fn screen_quantization_reports_each_candidate_and_rejects_invalid_scales() {
    let directory = unique_temp_directory();
    fs::create_dir(&directory).unwrap();
    let raw_path = directory.join("raw.bin");
    let fens_path = directory.join("suite.fen");
    fs::write(&raw_path, raw_fixture()).unwrap();
    fs::write(&fens_path, "4k3/8/8/8/3Q4/8/8/4K3 w - - 0 1\n").unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_screen-quantization"))
        .arg(&raw_path)
        .arg(&fens_path)
        .arg("255")
        .arg("8")
        .arg("2")
        .arg("64")
        .arg("128")
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"schema\":\"neyrang-nnue-quantization-screen-v1\""));
    assert!(stdout.contains("\"output_quant\":64"));
    assert!(stdout.contains("\"output_quant\":128"));
    assert!(stdout.contains("\"minimum_accumulator\":255"));
    assert!(stdout.contains("\"passed\":true"));

    let invalid = Command::new(env!("CARGO_BIN_EXE_screen-quantization"))
        .arg(&raw_path)
        .arg(&fens_path)
        .arg("255")
        .arg("8")
        .arg("2")
        .arg("0")
        .output()
        .unwrap();
    assert!(!invalid.status.success());
    assert!(
        String::from_utf8(invalid.stderr)
            .unwrap()
            .contains("output quantization must be positive")
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

fn unique_temp_directory() -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!(
        "neyrang-nnue-quantization-screen-{}-{nonce}",
        std::process::id()
    ))
}
