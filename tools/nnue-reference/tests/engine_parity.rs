use neyrang::{chess::Position, sanj::nnue::Network as EngineNetwork};
use neyrang_nnue_reference::{
    AccumulatorPair, FeatureSet, HIDDEN_SIZE, INPUT_FEATURES,
    INPUT_FEATURES_KING_BUCKETS_MIRRORED_3, Network as ReferenceNetwork, NetworkParameters,
    parse_fen_suite,
};

#[test]
fn engine_scalar_inference_is_bit_exact_with_the_reference_oracle() {
    let reference = deterministic_network();
    let artifact = reference.to_bytes();
    let engine = EngineNetwork::from_bytes(&artifact).expect("reference artifact must load");
    let fixtures = parse_fen_suite(include_str!("fixtures/n1d-parity.fen"))
        .expect("frozen parity suite must remain valid");

    for fixture in fixtures {
        let expected = reference.evaluate(
            &AccumulatorPair::refresh(&fixture.position, &reference),
            fixture.position.side_to_move(),
        );
        assert_eq!(
            engine.evaluate(&fixture.position),
            expected,
            "engine/reference drift at {}",
            fixture.position.to_fen()
        );
    }
}

#[test]
fn engine_scalar_inference_preserves_side_to_move_orientation() {
    let reference = deterministic_network();
    let engine =
        EngineNetwork::from_bytes(&reference.to_bytes()).expect("reference artifact must load");
    let white =
        Position::from_fen("4k3/8/8/8/8/8/8/3QK3 w - - 0 1").expect("fixture must be valid");
    let black =
        Position::from_fen("4k3/8/8/8/8/8/8/3QK3 b - - 0 1").expect("fixture must be valid");

    for position in [white, black] {
        let expected = reference.evaluate(
            &AccumulatorPair::refresh(&position, &reference),
            position.side_to_move(),
        );
        assert_eq!(engine.evaluate(&position), expected);
    }
}

#[test]
fn king_bucket_engine_inference_is_bit_exact_with_the_reference_oracle() {
    let reference = deterministic_king_bucket_network();
    let engine =
        EngineNetwork::from_bytes(&reference.to_bytes()).expect("version-two artifact must load");
    let positions = [
        Position::startpos(),
        Position::from_fen("r3k2r/8/8/8/8/8/8/R4RK1 w kq - 0 1").unwrap(),
        Position::from_fen("7k/8/8/3K4/8/8/8/8 b - - 0 1").unwrap(),
        Position::from_fen("8/8/8/8/3k4/8/8/4K3 w - - 0 1").unwrap(),
    ];

    for position in positions {
        let expected = reference.evaluate(
            &AccumulatorPair::refresh(&position, &reference),
            position.side_to_move(),
        );
        assert_eq!(
            engine.evaluate(&position),
            expected,
            "king-bucket engine/reference drift at {}",
            position.to_fen()
        );
    }
}

fn deterministic_network() -> ReferenceNetwork {
    ReferenceNetwork::new(
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
    .expect("fixture tensor shapes must be valid")
}

fn deterministic_king_bucket_network() -> ReferenceNetwork {
    ReferenceNetwork::new_with_feature_set(
        FeatureSet::Chess768KingBucketsMirrored3,
        NetworkParameters {
            activation_quant: 511,
            output_quant: 768,
            centipawn_scale: 400,
        },
        (0..INPUT_FEATURES_KING_BUCKETS_MIRRORED_3 * HIDDEN_SIZE)
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
    .expect("king-bucket fixture tensor shapes must be valid")
}
