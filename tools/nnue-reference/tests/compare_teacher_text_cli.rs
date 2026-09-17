use std::{
    fs,
    path::PathBuf,
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

use neyrang_nnue_reference::{HIDDEN_SIZE, INPUT_FEATURES, Network, NetworkParameters};

#[test]
fn compare_teacher_text_orients_white_scores_and_reports_exact_mse() {
    let directory = unique_temp_directory();
    fs::create_dir(&directory).unwrap();
    let baseline_path = directory.join("baseline.nnue");
    let candidate_path = directory.join("candidate.nnue");
    let corpus_path = directory.join("validation.txt");
    fs::write(&baseline_path, constant_network(100, 2, 1).to_bytes()).unwrap();
    fs::write(&candidate_path, constant_network(0, 1, 1).to_bytes()).unwrap();
    fs::write(
        &corpus_path,
        concat!(
            "# neyrang-teacher-wdl-logit400-v2\n",
            "4k3/8/8/8/8/8/8/4K3 w - - 0 1 | 0\n",
            "4k3/8/8/8/8/8/8/4K3 b - - 0 1 | 0\n",
        ),
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_compare-teacher-text"))
        .arg(&baseline_path)
        .arg(&candidate_path)
        .arg(&corpus_path)
        .arg("0")
        .arg("84")
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"schema\":\"neyrang-teacher-text-comparison-v1\""));
    assert!(stdout.contains("\"samples\":2"));
    assert!(stdout.contains("\"candidate_mse\":0.000000000000"));
    assert!(stdout.contains("\"candidate_bce\":0.693147180560"));
    assert!(stdout.contains("\"candidate_minus_baseline_bce\":-"));
    assert!(stdout.contains("\"candidate_best_mse_mix\":84"));
    assert!(stdout.contains("\"candidate_best_bce_mix\":84"));
    assert!(stdout.contains("\"baseline_selected_mix\":0"));
    assert!(stdout.contains("\"candidate_selected_mix\":84"));
    assert!(stdout.contains("\"improved_positions\":2"));

    fs::remove_dir_all(directory).unwrap();
}

fn constant_network(output_bias: i32, activation_quant: u16, output_quant: u16) -> Network {
    Network::new(
        NetworkParameters {
            activation_quant,
            output_quant,
            centipawn_scale: 400,
        },
        vec![0; INPUT_FEATURES * HIDDEN_SIZE],
        vec![0; HIDDEN_SIZE],
        vec![0; 2 * HIDDEN_SIZE],
        output_bias,
    )
    .unwrap()
}

fn unique_temp_directory() -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!(
        "neyrang-teacher-text-comparison-{}-{nonce}",
        std::process::id()
    ))
}
