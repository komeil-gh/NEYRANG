use std::{
    fs,
    path::PathBuf,
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

use neyrang_nnue_reference::{
    FeatureSet, HIDDEN_SIZE, INPUT_FEATURES, INPUT_FEATURES_KING_BUCKETS_MIRRORED_3, Network,
    NetworkParameters,
};

#[test]
fn importer_requires_explicit_quantization_and_preserves_it_in_the_artifact() {
    let directory = unique_temp_directory("quantization");
    fs::create_dir(&directory).unwrap();
    let input = directory.join("quantised.bin");
    let output = directory.join("network.nnue");
    fs::write(&input, bullet_fixture()).unwrap();

    let imported = Command::new(env!("CARGO_BIN_EXE_import-bullet"))
        .arg(&input)
        .arg(&output)
        .arg("511")
        .arg("768")
        .arg("400")
        .output()
        .unwrap();
    assert!(imported.status.success());
    let network = Network::from_bytes(&fs::read(&output).unwrap()).unwrap();
    assert_eq!(
        network.parameters(),
        NetworkParameters {
            activation_quant: 511,
            output_quant: 768,
            centipawn_scale: 400,
        }
    );

    let clobber = Command::new(env!("CARGO_BIN_EXE_import-bullet"))
        .arg(&input)
        .arg(&output)
        .arg("511")
        .arg("768")
        .arg("400")
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

#[test]
fn importer_requires_an_explicit_king_bucket_feature_contract() {
    let directory = unique_temp_directory("king-buckets");
    fs::create_dir(&directory).unwrap();
    let input = directory.join("quantised-x3.bin");
    let output = directory.join("network-x3.nnue");
    fs::write(
        &input,
        bullet_fixture_with_inputs(INPUT_FEATURES_KING_BUCKETS_MIRRORED_3),
    )
    .unwrap();

    let imported = Command::new(env!("CARGO_BIN_EXE_import-bullet"))
        .arg(&input)
        .arg(&output)
        .arg("511")
        .arg("768")
        .arg("400")
        .arg("--feature-set")
        .arg("chess768x3hm")
        .output()
        .unwrap();

    assert!(imported.status.success());
    let network = Network::from_bytes(&fs::read(&output).unwrap()).unwrap();
    assert_eq!(
        network.feature_set(),
        FeatureSet::Chess768KingBucketsMirrored3
    );
    fs::remove_dir_all(directory).unwrap();
}

fn bullet_fixture() -> Vec<u8> {
    bullet_fixture_with_inputs(INPUT_FEATURES)
}

fn bullet_fixture_with_inputs(inputs: usize) -> Vec<u8> {
    let mut bytes = vec![0; (inputs * HIDDEN_SIZE + HIDDEN_SIZE + 2 * HIDDEN_SIZE) * 2];
    bytes.extend_from_slice(&0_i32.to_le_bytes());
    let remainder = bytes.len() % 64;
    if remainder > 0 {
        for index in 0..64 - remainder {
            bytes.push(b"bullet"[index % 6]);
        }
    }
    bytes
}

fn unique_temp_directory(label: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!(
        "neyrang-nnue-import-{label}-{}-{nonce}",
        std::process::id()
    ))
}
