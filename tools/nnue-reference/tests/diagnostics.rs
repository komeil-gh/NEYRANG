use neyrang::chess::{Color, Position};
use neyrang_nnue_data::{Game, GameResult, ScoredMove};
use neyrang_nnue_reference::{
    HIDDEN_SIZE, INPUT_FEATURES, Network, NetworkDiagnosticError, NetworkParameters,
    blended_target, diagnose_network, logistic_cp,
};

#[test]
fn target_is_oriented_to_side_to_move_before_blending() {
    let white = blended_target(400, 1.0, Color::White, 0.75, 400.0);
    let black = blended_target(400, 1.0, Color::Black, 0.75, 400.0);

    assert!((white + black - 1.0).abs() < 1.0e-12);
    assert!(white > 0.9);
    assert!(black < 0.1);
}

#[test]
fn zero_centipawns_map_to_even_probability() {
    assert!((logistic_cp(0, 400.0) - 0.5).abs() < f64::EPSILON);
}

#[test]
fn corpus_diagnostic_measures_score_fit_and_filter_exposure() {
    let network = constant_network(0);
    let games = vec![
        game_with_score(
            "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
            "e2e4",
            400,
            GameResult::WhiteWin,
        ),
        game_with_score(
            "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR b KQkq - 0 1",
            "e7e5",
            400,
            GameResult::WhiteWin,
        ),
    ];

    let report = diagnose_network(&network, &games).unwrap();

    assert_eq!(report.games, 2);
    assert_eq!(report.positions, 2);
    assert_eq!(report.network_score_range_cp, (0, 0));
    assert_eq!(report.teacher_score_range_cp, (-400, 400));
    assert!((report.network_vs_search.mean_absolute_error_cp - 400.0).abs() < 1.0e-12);
    assert!((report.network_vs_search.root_mean_square_error_cp - 400.0).abs() < 1.0e-12);
    assert_eq!(report.network_vs_search.pearson_correlation, None);
    assert_eq!(report.network_vs_search.sign_samples, 2);
    assert_eq!(report.network_vs_search.sign_agreements, 0);
    assert_eq!(report.composition.early_ply, 2);
    assert_eq!(report.composition.tactical_move, 0);
    assert_eq!(report.composition.in_check, 0);
    assert_eq!(report.composition.bullet_default_rejected, 2);

    let score_probability = logistic_cp(400, 400.0);
    let score_error = (0.5 - score_probability).powi(2);
    assert!((report.score_probability_mse - score_error).abs() < 1.0e-12);
    assert!((report.result_probability_mse - 0.25).abs() < 1.0e-12);
}

#[test]
fn corpus_diagnostic_rejects_empty_corpora_and_empty_games() {
    let network = constant_network(0);
    assert_eq!(
        diagnose_network(&network, &[]),
        Err(NetworkDiagnosticError::EmptyCorpus)
    );

    let empty = Game {
        initial_position: Position::startpos(),
        header_score: 0,
        result: GameResult::Draw,
        extra: 0,
        moves: Vec::new(),
    };
    assert_eq!(
        diagnose_network(&network, &[empty]),
        Err(NetworkDiagnosticError::EmptyGame { index: 0 })
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

fn game_with_score(fen: &str, notation: &str, score_cp: i16, result: GameResult) -> Game {
    let mut initial_position = Position::from_fen(fen).unwrap();
    let mv = initial_position.find_legal_move(notation).unwrap();
    Game {
        initial_position,
        header_score: 0,
        result,
        extra: 0,
        moves: vec![ScoredMove { mv, score_cp }],
    }
}
