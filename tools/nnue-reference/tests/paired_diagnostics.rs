use neyrang::chess::Position;
use neyrang_nnue_data::{Game, GameResult, ScoredMove};
use neyrang_nnue_reference::{
    HIDDEN_SIZE, INPUT_FEATURES, Network, NetworkParameters, PairedComparisonError,
    compare_networks,
};

#[test]
fn paired_comparison_is_game_equal_deterministic_and_directional() {
    let baseline = constant_network(1);
    let candidate = constant_network(0);
    let games = vec![
        game_from_line(
            "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
            &["e2e4"],
        ),
        game_from_line(
            "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq - 0 1",
            &["e7e5", "g1f3"],
        ),
    ];

    let first = compare_networks(&baseline, &candidate, &games, 7, 1_000).unwrap();
    let repeat = compare_networks(&baseline, &candidate, &games, 7, 1_000).unwrap();

    assert_eq!(first, repeat);
    assert_eq!(first.games, 2);
    assert_eq!(first.positions, 3);
    assert!(first.candidate_mse < first.baseline_mse);
    assert!(first.position_weighted_delta_mse < 0.0);
    assert!(first.game_equal_mean_delta_mse < 0.0);
    assert_eq!(first.improved_games, 2);
    assert_eq!(first.tied_games, 0);
    assert_eq!(first.regressed_games, 0);
    assert!(first.bootstrap_lower_2_5_percentile < 0.0);
    assert!(first.bootstrap_upper_97_5_percentile < 0.0);
}

#[test]
fn paired_comparison_rejects_empty_input_zero_bootstrap_and_parameter_drift() {
    let network = constant_network(0);
    let game = game_from_line(
        "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
        &["e2e4"],
    );

    assert_eq!(
        compare_networks(&network, &network, &[], 1, 100),
        Err(PairedComparisonError::EmptyCorpus)
    );
    assert_eq!(
        compare_networks(&network, &network, std::slice::from_ref(&game), 1, 0),
        Err(PairedComparisonError::ZeroBootstrapReplicates)
    );

    let different = Network::new(
        NetworkParameters {
            activation_quant: 2,
            output_quant: 1,
            centipawn_scale: 400,
        },
        vec![0; INPUT_FEATURES * HIDDEN_SIZE],
        vec![0; HIDDEN_SIZE],
        vec![0; 2 * HIDDEN_SIZE],
        0,
    )
    .unwrap();
    assert_eq!(
        compare_networks(&network, &different, &[game], 1, 100),
        Err(PairedComparisonError::ParameterMismatch)
    );

    let empty_game = Game {
        initial_position: Position::startpos(),
        header_score: 0,
        result: GameResult::Draw,
        extra: 0,
        moves: Vec::new(),
    };
    assert_eq!(
        compare_networks(&network, &network, &[empty_game], 1, 100),
        Err(PairedComparisonError::EmptyGame { index: 0 })
    );
}

fn constant_network(output_bias: i32) -> Network {
    Network::new(
        NetworkParameters {
            activation_quant: 1,
            output_quant: 1,
            centipawn_scale: 400,
        },
        vec![0; INPUT_FEATURES * HIDDEN_SIZE],
        vec![0; HIDDEN_SIZE],
        vec![0; 2 * HIDDEN_SIZE],
        output_bias,
    )
    .unwrap()
}

fn game_from_line(fen: &str, line: &[&str]) -> Game {
    let initial_position = Position::from_fen(fen).unwrap();
    let mut position = initial_position.clone();
    let mut moves = Vec::new();
    for notation in line {
        let mv = position.find_legal_move(notation).unwrap();
        moves.push(ScoredMove { mv, score_cp: 0 });
        position.make_move(mv);
    }
    Game {
        initial_position,
        header_score: 0,
        result: GameResult::Draw,
        extra: 0,
        moves,
    }
}
