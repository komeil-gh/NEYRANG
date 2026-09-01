use std::{
    fs,
    path::PathBuf,
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use neyrang_nnue_reference::{HIDDEN_SIZE, INPUT_FEATURES, INPUT_FEATURES_KING_BUCKETS_MIRRORED_3};

#[test]
fn screen_quantization_reports_each_candidate_and_rejects_invalid_scales() {
    let directory = unique_temp_directory("legacy");
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

#[test]
fn screen_quantization_accepts_the_registered_king_bucket_feature_set() {
    let directory = unique_temp_directory("king-buckets");
    fs::create_dir(&directory).unwrap();
    let raw_path = directory.join("king-buckets.raw.bin");
    let fens_path = directory.join("suite.fen");
    fs::write(
        &raw_path,
        raw_fixture_with_inputs(INPUT_FEATURES_KING_BUCKETS_MIRRORED_3),
    )
    .unwrap();
    fs::write(&fens_path, "4k3/8/8/8/3Q4/8/8/4K3 w - - 0 1\n").unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_screen-quantization"))
        .arg(&raw_path)
        .arg(&fens_path)
        .arg("511")
        .arg("8")
        .arg("2")
        .arg("--feature-set")
        .arg("chess768x3hm")
        .arg("768")
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"output_quant\":768"));
    assert!(stdout.contains("\"passed\":true"));

    fs::remove_dir_all(directory).unwrap();
}

fn raw_fixture() -> Vec<u8> {
    raw_fixture_with_inputs(INPUT_FEATURES)
}

fn raw_fixture_with_inputs(input_features: usize) -> Vec<u8> {
    let float_count = input_features * HIDDEN_SIZE + HIDDEN_SIZE + 2 * HIDDEN_SIZE + 1;
    let mut values = vec![0.0_f32; float_count];
    let feature_bias_offset = input_features * HIDDEN_SIZE;
    let output_weight_offset = feature_bias_offset + HIDDEN_SIZE;
    values[feature_bias_offset..output_weight_offset].fill(1.0);
    values[output_weight_offset..output_weight_offset + 2 * HIDDEN_SIZE].fill(1.0 / 64.0);
    values
        .iter()
        .flat_map(|value| value.to_le_bytes())
        .collect()
}

fn unique_temp_directory(label: &str) -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!(
        "neyrang-nnue-quantization-screen-{label}-{}-{nonce}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed),
    ))
}
