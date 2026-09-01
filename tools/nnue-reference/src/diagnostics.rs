use neyrang::chess::Color;
use neyrang_nnue_data::{Game, GameResult};

use crate::{AccumulatorPair, Network};

const EVAL_SCALE: f64 = 400.0;
const WDL_PROPORTION: f64 = 0.75;

/// Paired fixed-corpus comparison with both position- and game-equal weighting.
#[derive(Clone, Debug, PartialEq)]
pub struct PairedComparisonReport {
    pub games: usize,
    pub positions: u64,
    pub baseline_mse: f64,
    pub candidate_mse: f64,
    pub position_weighted_delta_mse: f64,
    pub game_equal_mean_delta_mse: f64,
    pub improved_games: usize,
    pub tied_games: usize,
    pub regressed_games: usize,
    pub bootstrap_lower_2_5_percentile: f64,
    pub bootstrap_upper_97_5_percentile: f64,
    pub bootstrap_seed: u64,
    pub bootstrap_replicates: usize,
}

/// Fail-closed input errors for paired model diagnostics.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PairedComparisonError {
    EmptyCorpus,
    EmptyGame { index: usize },
    ZeroBootstrapReplicates,
    ParameterMismatch,
}

impl std::fmt::Display for PairedComparisonError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyCorpus => formatter.write_str("comparison corpus contains no games"),
            Self::EmptyGame { index } => {
                write!(
                    formatter,
                    "comparison game {index} contains no scored positions"
                )
            }
            Self::ZeroBootstrapReplicates => {
                formatter.write_str("bootstrap replicate count must be positive")
            }
            Self::ParameterMismatch => {
                formatter.write_str("network quantization or centipawn scale differs")
            }
        }
    }
}

impl std::error::Error for PairedComparisonError {}

/// Map a side-to-move centipawn score to the trainer's logistic probability.
#[must_use]
pub fn logistic_cp(score_cp: i32, eval_scale: f64) -> f64 {
    1.0 / (1.0 + (-(f64::from(score_cp) / eval_scale)).exp())
}

/// Reproduce Bullet's WDL/search-score target blend from white-relative labels.
#[must_use]
pub fn blended_target(
    score_white_cp: i16,
    result_white: f64,
    side_to_move: Color,
    wdl_proportion: f64,
    eval_scale: f64,
) -> f64 {
    let (score, result) = match side_to_move {
        Color::White => (i32::from(score_white_cp), result_white),
        Color::Black => (-i32::from(score_white_cp), 1.0 - result_white),
    };
    wdl_proportion * result + (1.0 - wdl_proportion) * logistic_cp(score, eval_scale)
}

/// Compare two identically scaled networks on the same complete games.
pub fn compare_networks(
    baseline: &Network,
    candidate: &Network,
    games: &[Game],
    bootstrap_seed: u64,
    bootstrap_replicates: usize,
) -> Result<PairedComparisonReport, PairedComparisonError> {
    if games.is_empty() {
        return Err(PairedComparisonError::EmptyCorpus);
    }
    if bootstrap_replicates == 0 {
        return Err(PairedComparisonError::ZeroBootstrapReplicates);
    }
    if baseline.parameters() != candidate.parameters() {
        return Err(PairedComparisonError::ParameterMismatch);
    }

    let mut total_positions = 0_u64;
    let mut baseline_squared_error = 0.0_f64;
    let mut candidate_squared_error = 0.0_f64;
    let mut game_deltas = Vec::with_capacity(games.len());
    for (game_index, game) in games.iter().enumerate() {
        if game.moves.is_empty() {
            return Err(PairedComparisonError::EmptyGame { index: game_index });
        }
        let result_white = match game.result {
            GameResult::BlackWin => 0.0,
            GameResult::Draw => 0.5,
            GameResult::WhiteWin => 1.0,
        };
        let mut position = game.initial_position.clone();
        let mut baseline_accumulators = AccumulatorPair::refresh(&position, baseline);
        let mut candidate_accumulators = AccumulatorPair::refresh(&position, candidate);
        let mut game_baseline_squared_error = 0.0_f64;
        let mut game_candidate_squared_error = 0.0_f64;
        for scored_move in &game.moves {
            let side_to_move = position.side_to_move();
            let target = blended_target(
                scored_move.score_cp,
                result_white,
                side_to_move,
                WDL_PROPORTION,
                EVAL_SCALE,
            );
            let baseline_prediction = logistic_cp(
                baseline.evaluate(&baseline_accumulators, side_to_move),
                EVAL_SCALE,
            );
            let candidate_prediction = logistic_cp(
                candidate.evaluate(&candidate_accumulators, side_to_move),
                EVAL_SCALE,
            );
            game_baseline_squared_error += (baseline_prediction - target).powi(2);
            game_candidate_squared_error += (candidate_prediction - target).powi(2);

            let before = position.clone();
            position.make_move(scored_move.mv);
            baseline_accumulators.update(&before, &position, baseline);
            candidate_accumulators.update(&before, &position, candidate);
        }
        let game_positions = game.moves.len() as f64;
        game_deltas.push(
            game_candidate_squared_error / game_positions
                - game_baseline_squared_error / game_positions,
        );
        total_positions += game.moves.len() as u64;
        baseline_squared_error += game_baseline_squared_error;
        candidate_squared_error += game_candidate_squared_error;
    }

    let position_count = total_positions as f64;
    let baseline_mse = baseline_squared_error / position_count;
    let candidate_mse = candidate_squared_error / position_count;
    let game_equal_mean_delta_mse = game_deltas.iter().sum::<f64>() / games.len() as f64;
    let improved_games = game_deltas.iter().filter(|&&delta| delta < 0.0).count();
    let tied_games = game_deltas.iter().filter(|&&delta| delta == 0.0).count();
    let regressed_games = games.len() - improved_games - tied_games;
    let (bootstrap_lower_2_5_percentile, bootstrap_upper_97_5_percentile) =
        bootstrap_interval(&game_deltas, bootstrap_seed, bootstrap_replicates);

    Ok(PairedComparisonReport {
        games: games.len(),
        positions: total_positions,
        baseline_mse,
        candidate_mse,
        position_weighted_delta_mse: candidate_mse - baseline_mse,
        game_equal_mean_delta_mse,
        improved_games,
        tied_games,
        regressed_games,
        bootstrap_lower_2_5_percentile,
        bootstrap_upper_97_5_percentile,
        bootstrap_seed,
        bootstrap_replicates,
    })
}

fn bootstrap_interval(deltas: &[f64], seed: u64, replicates: usize) -> (f64, f64) {
    let mut state = seed;
    let mut means = Vec::with_capacity(replicates);
    for _ in 0..replicates {
        let mut sum = 0.0_f64;
        for _ in 0..deltas.len() {
            let index = splitmix64(&mut state) as usize % deltas.len();
            sum += deltas[index];
        }
        means.push(sum / deltas.len() as f64);
    }
    means.sort_unstable_by(f64::total_cmp);
    (percentile(&means, 25), percentile(&means, 975))
}

fn percentile(sorted: &[f64], per_thousand: usize) -> f64 {
    let index = (sorted.len() - 1) * per_thousand / 1_000;
    sorted[index]
}

fn splitmix64(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9e37_79b9_7f4a_7c15);
    let mut value = *state;
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}
