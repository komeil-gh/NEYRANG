use std::{
    fs,
    path::PathBuf,
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use neyrang_nnue_reference::{
    FeatureSet, HIDDEN_SIZE, INPUT_FEATURES_KING_BUCKETS_MIRRORED_3, Network, NetworkParameters,
};

#[test]
fn quantize_raw_writes_one_valid_king_bucket_artifact_without_clobbering() {
    let directory = unique_temp_directory();
    fs::create_dir(&directory).unwrap();
    let raw_path = directory.join("network.raw.bin");
    let output_path = directory.join("network.nnue");
    fs::write(&raw_path, raw_fixture()).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_quantize-raw"))
        .arg(&raw_path)
        .arg(&output_path)
        .args(["1536", "768", "400", "--feature-set", "chess768x3hm"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let network = Network::from_bytes(&fs::read(&output_path).unwrap()).unwrap();
    assert_eq!(
        network.parameters(),
        NetworkParameters {
            activation_quant: 1536,
            output_quant: 768,
            centipawn_scale: 400,
        }
    );
    assert_eq!(
        network.feature_set(),
        FeatureSet::Chess768KingBucketsMirrored3
    );

    let clobber = Command::new(env!("CARGO_BIN_EXE_quantize-raw"))
        .arg(&raw_path)
        .arg(&output_path)
        .args(["1536", "768", "400", "--feature-set", "chess768x3hm"])
        .output()
        .unwrap();
    assert!(!clobber.status.success());
    assert!(
        String::from_utf8(clobber.stderr)
            .unwrap()
            .contains("refusing to overwrite")
    );

    fs::remove_dir_all(directory).unwrap();
}

fn raw_fixture() -> Vec<u8> {
    let float_count =
        INPUT_FEATURES_KING_BUCKETS_MIRRORED_3 * HIDDEN_SIZE + HIDDEN_SIZE + 2 * HIDDEN_SIZE + 1;
    let mut values = vec![0.0_f32; float_count];
    let feature_bias_offset = INPUT_FEATURES_KING_BUCKETS_MIRRORED_3 * HIDDEN_SIZE;
    let output_weight_offset = feature_bias_offset + HIDDEN_SIZE;
    values[feature_bias_offset..output_weight_offset].fill(1.0);
    values[output_weight_offset..output_weight_offset + 2 * HIDDEN_SIZE].fill(1.0 / 64.0);
    values
        .iter()
        .flat_map(|value| value.to_le_bytes())
        .collect()
}

fn unique_temp_directory() -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!(
        "neyrang-nnue-quantize-raw-{}-{nonce}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed),
    ))
}
