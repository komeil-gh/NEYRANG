use neyrang::chess::Position;
use neyrang_nnue_data::{Game, GameResult, ScoredMove};
use neyrang_nnue_reference::{
    HIDDEN_SIZE, INPUT_FEATURES, Network, NetworkParameters, diagnose_sign_disagreements,
};

#[test]
fn paired_sign_diagnostic_counts_directional_transitions_per_game() {
    let baseline = constant_network(1);
    let candidate = constant_network(-1);
    let games = vec![
        game_with_score(
            "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
            "e2e4",
            400,
        ),
        game_with_score(
            "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR b KQkq - 0 1",
            "e7e5",
            400,
        ),
    ];

    let report = diagnose_sign_disagreements(&baseline, &candidate, &games, 7, 1_000).unwrap();

    assert_eq!(report.games, 2);
    assert_eq!(report.signed_games, 2);
    assert_eq!(report.positions, 2);
    assert_eq!(report.overall.samples, 2);
    assert_eq!(report.overall.both_correct, 0);
    assert_eq!(report.overall.baseline_only_correct, 1);
    assert_eq!(report.overall.candidate_only_correct, 1);
    assert_eq!(report.overall.neither_correct, 0);
    assert_eq!(report.overall.baseline_agreements, 1);
    assert_eq!(report.overall.candidate_agreements, 1);
    assert_eq!(report.overall.candidate_minus_baseline_rate, 0.0);
    assert_eq!(report.game_equal_mean_delta_sign_agreement, 0.0);
    assert_eq!(report.improved_games, 1);
    assert_eq!(report.tied_games, 0);
    assert_eq!(report.regressed_games, 1);
    assert_eq!(report.bootstrap_seed, 7);
    assert_eq!(report.bootstrap_replicates, 1_000);

    assert_eq!(slice(&report.score_magnitude_bands, "201-400").samples, 2);
    assert_eq!(slice(&report.stm_king_buckets, "0").samples, 2);
    assert_eq!(slice(&report.ntm_king_buckets, "0").samples, 2);
    assert_eq!(slice(&report.king_bucket_pairs, "0:0").samples, 2);
    assert_eq!(slice(&report.piece_count_bands, "24-32").samples, 2);
    assert_eq!(slice(&report.side_to_move, "white").samples, 1);
    assert_eq!(slice(&report.side_to_move, "black").samples, 1);
    assert_eq!(slice(&report.contexts, "early_ply").samples, 2);
    assert_eq!(slice(&report.contexts, "not_tactical_move").samples, 2);
    assert_eq!(slice(&report.contexts, "not_in_check").samples, 2);
    assert_eq!(slice(&report.contexts, "not_castling_move").samples, 2);
}

fn slice<'a>(
    slices: &'a [neyrang_nnue_reference::NamedSignSliceReport],
    key: &str,
) -> &'a neyrang_nnue_reference::SignTransitionReport {
    &slices
        .iter()
        .find(|slice| slice.key == key)
        .unwrap()
        .transitions
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

fn game_with_score(fen: &str, notation: &str, score_cp: i16) -> Game {
    let initial_position = Position::from_fen(fen).unwrap();
    let mv = initial_position.clone().find_legal_move(notation).unwrap();
    Game {
        initial_position,
        header_score: 0,
        result: GameResult::Draw,
        extra: 0,
        moves: vec![ScoredMove { mv, score_cp }],
    }
}
