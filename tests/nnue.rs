#![cfg(feature = "nnue")]

use std::sync::atomic::AtomicBool;

use neyrang::{
    chess::Position,
    rekhne::{SearchLimits, Searcher, tt::TranspositionTable},
    sanj::{
        Evaluator,
        nnue::{AccumulatorPair, Network, NetworkError},
    },
    tools::bench,
};

const HIDDEN_SIZE: usize = 128;
const INPUT_FEATURES: usize = 2 * 6 * 64;
const KING_BUCKET_INPUT_FEATURES: usize = 3 * INPUT_FEATURES;

#[test]
fn scalar_network_decodes_and_evaluates_from_the_side_to_move() {
    let bytes = constant_network_artifact(32);
    let network = Network::from_bytes(&bytes).expect("fixture artifact must be valid");

    assert_eq!(network.evaluate(&Position::startpos()), 25);
}

#[test]
fn network_loader_rejects_legacy_magic_and_payload_corruption() {
    let bytes = constant_network_artifact(0);

    let mut legacy = bytes.clone();
    legacy[..8].copy_from_slice(b"OLDNNUE\0");
    assert_eq!(Network::from_bytes(&legacy), Err(NetworkError::BadMagic));

    let mut corrupt = bytes;
    corrupt[32] ^= 1;
    assert!(matches!(
        Network::from_bytes(&corrupt),
        Err(NetworkError::ChecksumMismatch { .. })
    ));
}

#[test]
fn version_two_king_bucket_artifact_uses_the_registered_mapping() {
    let home = Network::from_bytes(&king_bucket_probe_artifact(15))
        .expect("version-two king-bucket artifact must decode");
    let home_position = Position::from_fen("4k3/8/8/8/8/8/P7/4K3 w - - 0 1").unwrap();
    assert_eq!(home.evaluate(&home_position), 25);

    let castled = Network::from_bytes(&king_bucket_probe_artifact(INPUT_FEATURES + 15))
        .expect("version-two king-bucket artifact must decode");
    let castled_position = Position::from_fen("4k3/8/8/8/8/8/P7/5RK1 w - - 0 1").unwrap();
    assert_eq!(castled.evaluate(&castled_position), 25);
}

#[test]
fn version_two_accumulator_refreshes_after_king_moves() {
    let network = Network::from_bytes(&deterministic_king_bucket_artifact())
        .expect("version-two king-bucket artifact must decode");
    let cases = [
        ("4k3/8/8/8/8/8/8/4K3 w - - 0 1", "e1e2"),
        ("r3k2r/8/8/8/8/8/8/R3K2R w KQkq - 0 1", "e1g1"),
    ];

    for (fen, notation) in cases {
        let mut position = Position::from_fen(fen).unwrap();
        let mv = position.find_legal_move(notation).unwrap();
        let accumulator = AccumulatorPair::refresh(&position, &network);
        let next = accumulator.after_move(&position, mv, &network);
        position.make_move(mv);
        assert_eq!(
            next,
            AccumulatorPair::refresh(&position, &network),
            "king-bucket accumulator drift after {notation}"
        );
    }
}

#[test]
fn incremental_accumulator_matches_refresh_over_legal_play() {
    let network = Network::from_bytes(&deterministic_network_artifact())
        .expect("fixture artifact must be valid");

    for seed in 0..4_usize {
        let mut position = Position::startpos();
        let mut accumulator = AccumulatorPair::refresh(&position, &network);
        for ply in 0..64_usize {
            let moves = position.legal_moves();
            if moves.is_empty() {
                break;
            }
            let index =
                (position.hash() as usize ^ seed.wrapping_mul(131) ^ (ply * 17)) % moves.len();
            let mv = moves.as_slice()[index];
            let next = accumulator.after_move(&position, mv, &network);
            let undo = position.make_move(mv);

            assert_eq!(
                next,
                AccumulatorPair::refresh(&position, &network),
                "incremental mismatch at seed {seed}, ply {ply}, move {mv}"
            );
            assert_eq!(
                network.evaluate_accumulator(&next, position.side_to_move()),
                network.evaluate(&position),
                "score mismatch at seed {seed}, ply {ply}, move {mv}"
            );

            if ply % 9 == 0 {
                position.unmake_move(mv, undo);
                assert_eq!(accumulator, AccumulatorPair::refresh(&position, &network));
                let _ = position.make_move(mv);
            }
            accumulator = next;
        }
    }
}

#[test]
fn incremental_accumulator_handles_every_special_move_class() {
    let network = Network::from_bytes(&deterministic_network_artifact())
        .expect("fixture artifact must be valid");
    let cases = [
        ("r3k2r/8/8/8/8/8/8/R3K2R w KQkq - 0 1", "e1g1"),
        ("4k3/8/8/3pP3/8/8/8/4K3 w - d6 0 1", "e5d6"),
        ("4k3/P7/8/8/8/8/8/4K3 w - - 0 1", "a7a8q"),
        ("4k3/1r6/P7/8/8/8/8/4K3 w - - 0 1", "a6b7"),
    ];

    for (fen, notation) in cases {
        let mut position = Position::from_fen(fen).expect("fixture FEN must be valid");
        let mv = position
            .find_legal_move(notation)
            .unwrap_or_else(|| panic!("{notation} must be legal"));
        let accumulator = AccumulatorPair::refresh(&position, &network);
        let next = accumulator.after_move(&position, mv, &network);
        position.make_move(mv);
        assert_eq!(
            next,
            AccumulatorPair::refresh(&position, &network),
            "special-move mismatch after {notation}"
        );
    }
}

#[test]
fn search_can_use_nnue_without_changing_the_default_searcher_contract() {
    let network = Network::from_bytes(&deterministic_network_artifact())
        .expect("fixture artifact must be valid");
    let evaluator = Evaluator::nnue(network);
    let stop = AtomicBool::new(false);
    let mut searcher =
        Searcher::with_table_and_evaluator(&stop, TranspositionTable::new(1), evaluator);
    let mut position = Position::startpos();
    let original = position.clone();
    let hashes = [position.hash()];

    let result = searcher.search(&mut position, &SearchLimits::depth(3), &hashes, |_| {});

    assert!(result.best_move.is_some());
    assert!(result.nodes > 0);
    assert_eq!(position, original);
}

#[cfg(feature = "stats")]
#[test]
fn nnue_accumulator_stack_survives_null_move_pruning() {
    let network = Network::from_bytes(&deterministic_network_artifact())
        .expect("fixture artifact must be valid");
    let stop = AtomicBool::new(false);
    let mut searcher = Searcher::with_table_and_evaluator(
        &stop,
        TranspositionTable::new(4),
        Evaluator::nnue(network),
    );
    let mut position = Position::startpos();
    let original = position.clone();
    let hashes = [position.hash()];

    let result = searcher.search(&mut position, &SearchLimits::depth(7), &hashes, |_| {});

    assert!(result.statistics.null_move_attempts > 0);
    assert_eq!(position, original);
}

#[test]
fn nnue_search_benchmark_is_deterministic() {
    let network = Network::from_bytes(&deterministic_network_artifact())
        .expect("fixture artifact must be valid");
    let evaluator = Evaluator::nnue(network);

    let first = bench::run_with_evaluator(2, evaluator.clone()).expect("benchmark must run");
    let second = bench::run_with_evaluator(2, evaluator).expect("benchmark must repeat");

    assert_eq!(first.positions, 5);
    assert!(first.nodes > 0);
    assert_eq!(first.nodes, second.nodes);
    assert_eq!(first.checksum, second.checksum);
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

fn deterministic_network_artifact() -> Vec<u8> {
    let feature_weight_count = INPUT_FEATURES * HIDDEN_SIZE;
    let feature_bias_count = HIDDEN_SIZE;
    let output_weight_count = 2 * HIDDEN_SIZE;
    let payload_size = (feature_weight_count + feature_bias_count + output_weight_count) * 2 + 4;
    let mut payload = Vec::with_capacity(payload_size);
    for index in 0..feature_weight_count {
        payload.extend_from_slice(&((index as i16 % 31) - 15).to_le_bytes());
    }
    for index in 0..feature_bias_count {
        payload.extend_from_slice(&((index as i16 % 13) - 6).to_le_bytes());
    }
    for index in 0..output_weight_count {
        payload.extend_from_slice(&((index as i16 % 17) - 8).to_le_bytes());
    }
    payload.extend_from_slice(&19_i32.to_le_bytes());

    let mut bytes = Vec::with_capacity(32 + payload_size);
    bytes.extend_from_slice(b"NEYRANG\0");
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&(HIDDEN_SIZE as u16).to_le_bytes());
    bytes.extend_from_slice(&511_u16.to_le_bytes());
    bytes.extend_from_slice(&768_u16.to_le_bytes());
    bytes.extend_from_slice(&0_u16.to_le_bytes());
    bytes.extend_from_slice(&400_i32.to_le_bytes());
    bytes.extend_from_slice(&(payload_size as u32).to_le_bytes());
    bytes.extend_from_slice(&crc32(&payload).to_le_bytes());
    bytes.extend_from_slice(&payload);
    bytes
}

fn king_bucket_probe_artifact(active_feature: usize) -> Vec<u8> {
    let feature_weight_count = KING_BUCKET_INPUT_FEATURES * HIDDEN_SIZE;
    let feature_bias_count = HIDDEN_SIZE;
    let output_weight_count = 2 * HIDDEN_SIZE;
    let payload_size = (feature_weight_count + feature_bias_count + output_weight_count) * 2 + 4;
    let mut payload = vec![0; payload_size];
    let weight_offset = (active_feature * HIDDEN_SIZE) * 2;
    payload[weight_offset..weight_offset + 2].copy_from_slice(&8_i16.to_le_bytes());
    let output_offset = (feature_weight_count + feature_bias_count) * 2;
    payload[output_offset..output_offset + 2].copy_from_slice(&16_i16.to_le_bytes());
    version_two_artifact(payload)
}

fn deterministic_king_bucket_artifact() -> Vec<u8> {
    let feature_weight_count = KING_BUCKET_INPUT_FEATURES * HIDDEN_SIZE;
    let feature_bias_count = HIDDEN_SIZE;
    let output_weight_count = 2 * HIDDEN_SIZE;
    let payload_size = (feature_weight_count + feature_bias_count + output_weight_count) * 2 + 4;
    let mut payload = Vec::with_capacity(payload_size);
    for index in 0..feature_weight_count {
        payload.extend_from_slice(&((index as i16 % 31) - 15).to_le_bytes());
    }
    for index in 0..feature_bias_count {
        payload.extend_from_slice(&((index as i16 % 13) - 6).to_le_bytes());
    }
    for index in 0..output_weight_count {
        payload.extend_from_slice(&((index as i16 % 17) - 8).to_le_bytes());
    }
    payload.extend_from_slice(&19_i32.to_le_bytes());
    version_two_artifact(payload)
}

fn version_two_artifact(payload: Vec<u8>) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(32 + payload.len());
    bytes.extend_from_slice(b"NEYRANG\0");
    bytes.extend_from_slice(&2_u16.to_le_bytes());
    bytes.extend_from_slice(&2_u16.to_le_bytes());
    bytes.extend_from_slice(&(HIDDEN_SIZE as u16).to_le_bytes());
    bytes.extend_from_slice(&32_u16.to_le_bytes());
    bytes.extend_from_slice(&16_u16.to_le_bytes());
    bytes.extend_from_slice(&0_u16.to_le_bytes());
    bytes.extend_from_slice(&400_i32.to_le_bytes());
    bytes.extend_from_slice(&(payload.len() as u32).to_le_bytes());
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
