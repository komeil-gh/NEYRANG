use neyrang::chess::{Color, PieceType, Position, Square};
use neyrang_nnue_reference::{
    AccumulatorPair, FeatureSet, FenSuiteError, FloatNetwork, FloatNetworkError,
    FloatQuantizationError, HIDDEN_SIZE, INPUT_FEATURES, INPUT_FEATURES_KING_BUCKETS_MIRRORED_3,
    Network, NetworkParameters, ParityError, active_features, active_features_for, evaluate_parity,
    parse_fen_suite,
};

const FLOAT_COUNT: usize = INPUT_FEATURES * HIDDEN_SIZE + HIDDEN_SIZE + 2 * HIDDEN_SIZE + 1;

#[test]
fn bullet_raw_float_stream_evaluates_the_documented_screlu_topology() {
    let mut values = vec![0.0_f32; FLOAT_COUNT];
    let feature_bias_offset = INPUT_FEATURES * HIDDEN_SIZE;
    let output_weight_offset = feature_bias_offset + HIDDEN_SIZE;

    values[feature_bias_offset..output_weight_offset].fill(1.0);
    values[output_weight_offset..output_weight_offset + 2 * HIDDEN_SIZE].fill(1.0 / 64.0);

    let network = FloatNetwork::from_bullet_raw(&encode_f32(&values), 400.0)
        .expect("fixture has the exact Bullet raw layout");

    assert_eq!(network.evaluate(&Position::startpos()), 1600.0);
}

#[test]
fn float_evaluation_orients_the_two_perspectives_by_side_to_move() {
    let mut values = vec![0.0_f32; FLOAT_COUNT];
    let output_weight_offset = INPUT_FEATURES * HIDDEN_SIZE + HIDDEN_SIZE;
    let queen_d4 = neyrang_nnue_reference::feature_index(
        Color::White,
        PieceType::Queen,
        Square::D4,
        Color::White,
    );
    values[queen_d4 * HIDDEN_SIZE] = 1.0;
    values[output_weight_offset] = 1.0 / 64.0;
    values[output_weight_offset + HIDDEN_SIZE] = -1.0 / 64.0;

    let network = FloatNetwork::from_bullet_raw(&encode_f32(&values), 400.0).unwrap();
    let white = Position::from_fen("4k3/8/8/8/3Q4/8/8/4K3 w - - 0 1").unwrap();
    let black = Position::from_fen("4k3/8/8/8/3Q4/8/8/4K3 b - - 0 1").unwrap();

    assert_eq!(network.evaluate(&white), 6.25);
    assert_eq!(network.evaluate(&black), -6.25);
}

#[test]
fn raw_stream_rejects_wrong_length_and_non_finite_weights() {
    let valid = vec![0.0_f32; FLOAT_COUNT];
    let mut truncated = encode_f32(&valid);
    truncated.pop();
    assert!(matches!(
        FloatNetwork::from_bullet_raw(&truncated, 400.0),
        Err(FloatNetworkError::LengthMismatch { .. })
    ));

    let mut non_finite = valid;
    non_finite[17] = f32::NAN;
    assert_eq!(
        FloatNetwork::from_bullet_raw(&encode_f32(&non_finite), 400.0),
        Err(FloatNetworkError::NonFiniteWeight { index: 17 })
    );
    assert_eq!(
        FloatNetwork::from_bullet_raw(&encode_f32(&vec![0.0; FLOAT_COUNT]), 0.0),
        Err(FloatNetworkError::InvalidCentipawnScale)
    );
}

#[test]
fn raw_quantization_reproduces_bullets_rounding_and_tensor_order() {
    let mut values = vec![0.0_f32; FLOAT_COUNT];
    let feature_bias_offset = INPUT_FEATURES * HIDDEN_SIZE;
    let output_weight_offset = feature_bias_offset + HIDDEN_SIZE;
    let output_bias_index = output_weight_offset + 2 * HIDDEN_SIZE;
    values[0] = 0.5;
    values[feature_bias_offset] = -0.5;
    values[output_weight_offset] = 0.5;
    values[output_bias_index] = -0.5;
    let raw = FloatNetwork::from_bullet_raw(&encode_f32(&values), 400.0).unwrap();
    let parameters = NetworkParameters {
        activation_quant: 255,
        output_quant: 64,
        centipawn_scale: 400,
    };

    let actual = raw.quantize(parameters).unwrap();
    let mut feature_weights = vec![0; INPUT_FEATURES * HIDDEN_SIZE];
    feature_weights[0] = 128;
    let mut feature_bias = vec![0; HIDDEN_SIZE];
    feature_bias[0] = -128;
    let mut output_weights = vec![0; 2 * HIDDEN_SIZE];
    output_weights[0] = 32;
    let expected = Network::new(
        parameters,
        feature_weights,
        feature_bias,
        output_weights,
        -8160,
    )
    .unwrap();

    assert_eq!(actual, expected);
}

#[test]
fn raw_quantization_fails_instead_of_saturating_out_of_range_weights() {
    let mut values = vec![0.0_f32; FLOAT_COUNT];
    values[7] = 200.0;
    let raw = FloatNetwork::from_bullet_raw(&encode_f32(&values), 400.0).unwrap();

    assert_eq!(
        raw.quantize(NetworkParameters {
            activation_quant: 255,
            output_quant: 64,
            centipawn_scale: 400,
        }),
        Err(FloatQuantizationError::I16OutOfRange {
            tensor: "feature_weights",
            index: 7,
        })
    );
}

#[test]
fn parity_report_compares_raw_and_quantized_scores_on_real_positions() {
    let mut raw_values = vec![0.0_f32; FLOAT_COUNT];
    let feature_bias_offset = INPUT_FEATURES * HIDDEN_SIZE;
    let output_weight_offset = feature_bias_offset + HIDDEN_SIZE;
    raw_values[feature_bias_offset..output_weight_offset].fill(1.0);
    raw_values[output_weight_offset..output_weight_offset + 2 * HIDDEN_SIZE].fill(1.0 / 64.0);
    let raw = FloatNetwork::from_bullet_raw(&encode_f32(&raw_values), 400.0).unwrap();

    let quantized = Network::new(
        NetworkParameters {
            activation_quant: 255,
            output_quant: 64,
            centipawn_scale: 400,
        },
        vec![0; INPUT_FEATURES * HIDDEN_SIZE],
        vec![255; HIDDEN_SIZE],
        vec![1; 2 * HIDDEN_SIZE],
        0,
    )
    .unwrap();
    let positions = [
        Position::startpos(),
        Position::from_fen("4k3/8/8/8/3Q4/8/8/4K3 b - - 0 1").unwrap(),
    ];

    let report = evaluate_parity(&raw, &quantized, &positions).unwrap();

    assert_eq!(report.samples.len(), 2);
    assert_eq!(report.max_abs_error_cp, 0.0);
    assert_eq!(report.mean_abs_error_cp, 0.0);
    assert_eq!(report.minimum_accumulator, 255);
    assert_eq!(report.maximum_accumulator, 255);
    assert!(report.passes(8.0, 2.0));
    assert!(!report.passes(-1.0, 2.0));
}

#[test]
fn active_feature_fixture_really_distinguishes_the_two_perspectives() {
    let position = Position::from_fen("4k3/8/8/8/3Q4/8/8/4K3 w - - 0 1").unwrap();
    let white = active_features(&position, Color::White);
    let black = active_features(&position, Color::Black);

    assert_ne!(white, black);
}

#[test]
fn king_bucket_raw_tensor_quantizes_with_the_same_feature_contract() {
    let float_count =
        INPUT_FEATURES_KING_BUCKETS_MIRRORED_3 * HIDDEN_SIZE + HIDDEN_SIZE + 2 * HIDDEN_SIZE + 1;
    let raw = encode_f32(&vec![0.0; float_count]);
    let float = FloatNetwork::from_bullet_raw_with_feature_set(
        &raw,
        400.0,
        FeatureSet::Chess768KingBucketsMirrored3,
    )
    .unwrap();
    let quantized = float
        .quantize(NetworkParameters {
            activation_quant: 511,
            output_quant: 768,
            centipawn_scale: 400,
        })
        .unwrap();

    assert_eq!(
        quantized.feature_set(),
        FeatureSet::Chess768KingBucketsMirrored3
    );
    assert_eq!(float.evaluate(&Position::startpos()), 0.0);
}

#[test]
fn phase_head_raw_tensor_routes_by_material_and_survives_quantization() {
    let float_count = INPUT_FEATURES_KING_BUCKETS_MIRRORED_3 * HIDDEN_SIZE
        + HIDDEN_SIZE
        + 4 * 2 * HIDDEN_SIZE
        + 4;
    let mut values = vec![0.0; float_count];
    values[float_count - 4..].copy_from_slice(&[1.0, 2.0, 3.0, 4.0]);
    let float = FloatNetwork::from_bullet_raw_with_feature_set(
        &encode_f32(&values),
        400.0,
        FeatureSet::Chess768KingBucketsMirrored3PhaseHeads4,
    )
    .unwrap();
    let quantized = float
        .quantize(NetworkParameters {
            activation_quant: 511,
            output_quant: 768,
            centipawn_scale: 400,
        })
        .unwrap();
    let bare_kings = Position::from_fen("4k3/8/8/8/8/8/8/4K3 w - - 0 1").unwrap();
    let start = Position::startpos();

    assert_eq!(float.evaluate(&bare_kings), 400.0);
    assert_eq!(float.evaluate(&start), 1600.0);
    for (position, expected) in [(&bare_kings, 400), (&start, 1600)] {
        let accumulators = AccumulatorPair::refresh(position, &quantized);
        assert_eq!(
            quantized.evaluate(&accumulators, position.side_to_move()),
            expected
        );
    }
}

#[test]
fn latent_imbalance_channel_survives_quantization() {
    let feature_set = FeatureSet::Chess768KingBucketsMirrored3LatentImbalance;
    let feature_weight_count = INPUT_FEATURES_KING_BUCKETS_MIRRORED_3 * HIDDEN_SIZE;
    let output_weight_offset = feature_weight_count + HIDDEN_SIZE;
    let float_count = output_weight_offset + 3 * HIDDEN_SIZE + 1;
    let position = Position::from_fen("4k3/8/8/8/3Q4/8/8/4K3 w - - 0 1").unwrap();
    let white = active_features_for(&position, Color::White, feature_set);
    let black = active_features_for(&position, Color::Black, feature_set);
    let asymmetric_feature = *white
        .iter()
        .find(|feature| !black.contains(feature))
        .unwrap();
    let mut values = vec![0.0; float_count];
    values[asymmetric_feature * HIDDEN_SIZE] = 1.0;
    values[output_weight_offset + 2 * HIDDEN_SIZE] = 1.0;

    let float =
        FloatNetwork::from_bullet_raw_with_feature_set(&encode_f32(&values), 400.0, feature_set)
            .unwrap();
    let quantized = float
        .quantize(NetworkParameters {
            activation_quant: 511,
            output_quant: 768,
            centipawn_scale: 400,
        })
        .unwrap();
    let accumulators = AccumulatorPair::refresh(&position, &quantized);

    assert_eq!(float.evaluate(&position), 400.0);
    assert_eq!(
        quantized.evaluate(&accumulators, position.side_to_move()),
        400
    );
}

#[test]
fn parity_rejects_mismatched_feature_contracts() {
    let float_count =
        INPUT_FEATURES_KING_BUCKETS_MIRRORED_3 * HIDDEN_SIZE + HIDDEN_SIZE + 2 * HIDDEN_SIZE + 1;
    let float = FloatNetwork::from_bullet_raw_with_feature_set(
        &encode_f32(&vec![0.0; float_count]),
        400.0,
        FeatureSet::Chess768KingBucketsMirrored3,
    )
    .unwrap();
    let quantized = Network::new(
        NetworkParameters {
            activation_quant: 511,
            output_quant: 768,
            centipawn_scale: 400,
        },
        vec![0; INPUT_FEATURES * HIDDEN_SIZE],
        vec![0; HIDDEN_SIZE],
        vec![0; 2 * HIDDEN_SIZE],
        0,
    )
    .unwrap();

    assert_eq!(
        evaluate_parity(&float, &quantized, &[Position::startpos()]),
        Err(ParityError::FeatureSetMismatch)
    );
}

#[test]
fn frozen_fen_suite_ignores_comments_and_preserves_full_fens() {
    let suite = parse_fen_suite(
        "# side-to-move coverage\n\n\
         4k3/8/8/8/3Q4/8/8/4K3 w - - 0 1\n\
         4k3/8/8/8/3Q4/8/8/4K3 b - - 0 1\n",
    )
    .unwrap();

    assert_eq!(suite.len(), 2);
    assert!(suite[0].fen.ends_with(" w - - 0 1"));
    assert_eq!(suite[1].position.side_to_move(), Color::Black);
}

#[test]
fn frozen_fen_suite_rejects_duplicates_invalid_fens_and_empty_input() {
    let duplicate = concat!(
        "4k3/8/8/8/3Q4/8/8/4K3 w - - 0 1\n",
        "4k3/8/8/8/3Q4/8/8/4K3 w - - 9 42\n",
    );
    assert!(matches!(
        parse_fen_suite(duplicate),
        Err(FenSuiteError::DuplicatePosition { line: 2 })
    ));
    assert!(matches!(
        parse_fen_suite("not a fen\n"),
        Err(FenSuiteError::InvalidFen { line: 1, .. })
    ));
    assert!(matches!(
        parse_fen_suite("# only a comment\n"),
        Err(FenSuiteError::Empty)
    ));
}

fn encode_f32(values: &[f32]) -> Vec<u8> {
    values
        .iter()
        .flat_map(|value| value.to_le_bytes())
        .collect()
}
