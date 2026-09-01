use neyrang::chess::{Color, Position};
use neyrang_nnue_reference::{
    AccumulatorPair, FeatureSet, HIDDEN_SIZE, INPUT_FEATURES,
    INPUT_FEATURES_KING_BUCKETS_MIRRORED_3, Network, NetworkParameters, feature_index,
};

fn deterministic_network() -> Network {
    let feature_weights = (0..INPUT_FEATURES * HIDDEN_SIZE)
        .map(|index| (index as i16 % 7) - 3)
        .collect();
    let feature_bias = (0..HIDDEN_SIZE)
        .map(|index| (index as i16 % 5) - 2)
        .collect();
    let output_weights = (0..2 * HIDDEN_SIZE)
        .map(|index| (index as i16 % 9) - 4)
        .collect();

    Network::new(
        NetworkParameters {
            activation_quant: 255,
            output_quant: 64,
            centipawn_scale: 400,
        },
        feature_weights,
        feature_bias,
        output_weights,
        13,
    )
    .expect("fixture shape is valid")
}

fn make_uci(position: &mut Position, uci: &str) -> neyrang::chess::UndoState {
    let mv = position
        .legal_moves()
        .iter()
        .copied()
        .find(|mv| mv.to_string() == uci)
        .unwrap_or_else(|| panic!("{uci} must be legal in the fixture"));
    position.make_move(mv)
}

#[test]
fn incremental_accumulator_matches_refresh_over_long_legal_play_and_undo() {
    let network = deterministic_network();

    for seed in 0..8_usize {
        let mut position = Position::startpos();
        let mut accumulator = AccumulatorPair::refresh(&position, &network);

        for ply in 0..96_usize {
            let moves = position.legal_moves();
            if moves.is_empty() {
                break;
            }
            let index = (position.hash() as usize ^ seed.wrapping_mul(131) ^ ply.wrapping_mul(17))
                % moves.len();
            let mv = moves.as_slice()[index];
            let before = position.clone();
            let undo = position.make_move(mv);

            accumulator.update(&before, &position, &network);
            assert_eq!(
                accumulator,
                AccumulatorPair::refresh(&position, &network),
                "refresh mismatch at seed {seed}, ply {ply}, move {mv}"
            );

            if ply % 11 == 0 {
                let after = position.clone();
                position.unmake_move(mv, undo);
                accumulator.update(&after, &position, &network);
                assert_eq!(accumulator, AccumulatorPair::refresh(&before, &network));

                let redo_undo = position.make_move(mv);
                accumulator.update(&before, &position, &network);
                assert_eq!(accumulator, AccumulatorPair::refresh(&position, &network));
                let _ = redo_undo;
            }
        }
    }
}

#[test]
fn special_moves_and_captures_match_full_refresh() {
    let network = deterministic_network();
    let cases = [
        ("r3k2r/8/8/8/8/8/8/R3K2R w KQkq - 0 1", "e1g1"),
        ("4k3/8/8/3pP3/8/8/8/4K3 w - d6 0 1", "e5d6"),
        ("4k3/P7/8/8/8/8/8/4K3 w - - 0 1", "a7a8q"),
        ("4k3/8/8/8/8/8/4r3/4K3 w - - 0 1", "e1e2"),
        ("4k3/8/8/8/8/8/8/4K3 w - - 0 1", "e1f1"),
    ];

    for (fen, uci) in cases {
        let mut position = Position::from_fen(fen).expect("fixture FEN is valid");
        let before = position.clone();
        let mut accumulator = AccumulatorPair::refresh(&before, &network);
        let undo = make_uci(&mut position, uci);
        let mv = before
            .clone()
            .legal_moves()
            .iter()
            .copied()
            .find(|mv| mv.to_string() == uci)
            .expect("fixture move remains discoverable");

        accumulator.update(&before, &position, &network);
        assert_eq!(
            accumulator,
            AccumulatorPair::refresh(&position, &network),
            "refresh mismatch after {uci}"
        );

        let after = position.clone();
        position.unmake_move(mv, undo);
        accumulator.update(&after, &position, &network);
        assert_eq!(
            accumulator,
            AccumulatorPair::refresh(&before, &network),
            "undo mismatch after {uci}"
        );
    }
}

#[test]
fn evaluation_orders_accumulators_by_side_to_move() {
    let mut feature_weights = vec![0; INPUT_FEATURES * HIDDEN_SIZE];
    let white_queen = feature_index(
        Color::White,
        neyrang::chess::PieceType::Queen,
        neyrang::chess::Square::D1,
        Color::White,
    );
    feature_weights[white_queen * HIDDEN_SIZE] = 8;

    let mut output_weights = vec![0; 2 * HIDDEN_SIZE];
    output_weights[0] = 16;
    output_weights[HIDDEN_SIZE] = 4;
    let network = Network::new(
        NetworkParameters {
            activation_quant: 32,
            output_quant: 16,
            centipawn_scale: 400,
        },
        feature_weights,
        vec![0; HIDDEN_SIZE],
        output_weights,
        0,
    )
    .expect("fixture shape is valid");

    let white = Position::from_fen("4k3/8/8/8/8/8/8/3QK3 w - - 0 1")
        .expect("white-to-move fixture is valid");
    let black = Position::from_fen("4k3/8/8/8/8/8/8/3QK3 b - - 0 1")
        .expect("black-to-move fixture is valid");
    let accumulators = AccumulatorPair::refresh(&white, &network);

    let white_score = network.evaluate(&accumulators, white.side_to_move());
    let black_score = network.evaluate(&accumulators, black.side_to_move());

    assert!(white_score > black_score);
    assert!(black_score >= 0);
}

#[test]
fn extreme_valid_quantization_values_have_defined_saturating_output() {
    let network = Network::new(
        NetworkParameters {
            activation_quant: u16::MAX,
            output_quant: 1,
            centipawn_scale: i32::MAX,
        },
        vec![i16::MAX; INPUT_FEATURES * HIDDEN_SIZE],
        vec![i16::MAX; HIDDEN_SIZE],
        vec![i16::MAX; 2 * HIDDEN_SIZE],
        i32::MAX,
    )
    .expect("extreme values are representable by the artifact format");
    let position = Position::startpos();
    let accumulators = AccumulatorPair::refresh(&position, &network);

    assert_eq!(
        network.evaluate(&accumulators, position.side_to_move()),
        i32::MAX
    );
}

#[test]
fn king_bucket_accumulator_refreshes_when_the_king_changes_bank() {
    let network = Network::new_with_feature_set(
        FeatureSet::Chess768KingBucketsMirrored3,
        NetworkParameters {
            activation_quant: 511,
            output_quant: 768,
            centipawn_scale: 400,
        },
        (0..INPUT_FEATURES_KING_BUCKETS_MIRRORED_3 * HIDDEN_SIZE)
            .map(|index| (index as i16 % 31) - 15)
            .collect(),
        vec![0; HIDDEN_SIZE],
        vec![1; 2 * HIDDEN_SIZE],
        0,
    )
    .unwrap();
    let mut position = Position::from_fen("4k3/8/8/8/8/8/8/4K3 w - - 0 1").unwrap();
    let before = position.clone();
    let mut accumulator = AccumulatorPair::refresh(&position, &network);
    let mv = position.find_legal_move("e1e2").unwrap();
    position.make_move(mv);

    accumulator.update(&before, &position, &network);

    assert_eq!(accumulator, AccumulatorPair::refresh(&position, &network));
}
