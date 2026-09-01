use std::{
    fs,
    path::PathBuf,
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

use neyrang_nnue_reference::{HIDDEN_SIZE, INPUT_FEATURES, Network, NetworkParameters};

#[test]
fn importer_requires_explicit_quantization_and_preserves_it_in_the_artifact() {
    let directory = unique_temp_directory();
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

fn bullet_fixture() -> Vec<u8> {
    let mut bytes = vec![0; (INPUT_FEATURES * HIDDEN_SIZE + HIDDEN_SIZE + 2 * HIDDEN_SIZE) * 2];
    bytes.extend_from_slice(&0_i32.to_le_bytes());
    let remainder = bytes.len() % 64;
    if remainder > 0 {
        for index in 0..64 - remainder {
            bytes.push(b"bullet"[index % 6]);
        }
    }
    bytes
}

fn unique_temp_directory() -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("neyrang-nnue-import-{}-{nonce}", std::process::id()))
}
