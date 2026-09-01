use std::{
    fs,
    path::PathBuf,
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

use neyrang_nnue_reference::{HIDDEN_SIZE, INPUT_FEATURES, Network, NetworkParameters};

#[test]
fn verify_engine_parity_emits_machine_readable_bit_exact_evidence() {
    let directory = unique_temp_directory();
    fs::create_dir(&directory).unwrap();
    let network_path = directory.join("network.nnue");
    let fens_path = directory.join("suite.fen");
    fs::write(&network_path, fixture_network().to_bytes()).unwrap();
    fs::write(
        &fens_path,
        concat!(
            "4k3/8/8/8/3Q4/8/8/4K3 w - - 0 1\n",
            "4k3/8/8/8/3Q4/8/8/4K3 b - - 0 1\n",
        ),
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_verify-engine-parity"))
        .arg(&network_path)
        .arg(&fens_path)
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"schema\":\"neyrang-nnue-engine-parity-v1\""));
    assert!(stdout.contains("\"positions\":2"));
    assert!(stdout.contains("\"mismatches\":0"));
    assert!(stdout.contains("\"passed\":true"));

    fs::remove_dir_all(directory).unwrap();
}

fn fixture_network() -> Network {
    Network::new(
        NetworkParameters {
            activation_quant: 511,
            output_quant: 768,
            centipawn_scale: 400,
        },
        (0..INPUT_FEATURES * HIDDEN_SIZE)
            .map(|index| (index as i16 % 31) - 15)
            .collect(),
        (0..HIDDEN_SIZE)
            .map(|index| (index as i16 % 13) - 6)
            .collect(),
        (0..2 * HIDDEN_SIZE)
            .map(|index| (index as i16 % 17) - 8)
            .collect(),
        19,
    )
    .unwrap()
}

fn unique_temp_directory() -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!(
        "neyrang-nnue-engine-parity-{}-{nonce}",
        std::process::id()
    ))
}
