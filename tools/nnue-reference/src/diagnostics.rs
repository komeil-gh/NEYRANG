use neyrang::{
    chess::{Color, Position},
    sanj,
};
use neyrang_nnue_data::{Game, GameResult};

use crate::{AccumulatorPair, Network, king_bucket_mirrored_3};

const EVAL_SCALE: f64 = 400.0;
const WDL_PROPORTION: f64 = 0.75;

/// Linear score-fit statistics in centipawns.
#[derive(Clone, Debug, PartialEq)]
pub struct ScoreFitReport {
    pub samples: u64,
    pub reference_mean_cp: f64,
    pub prediction_mean_cp: f64,
    pub mean_error_cp: f64,
    pub mean_absolute_error_cp: f64,
    pub root_mean_square_error_cp: f64,
    pub pearson_correlation: Option<f64>,
    pub sign_samples: u64,
    pub sign_agreements: u64,
    pub sign_agreement_rate: f64,
}

/// Deterministic exposure to the position filters recommended by Viriformat/Bullet.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CorpusCompositionReport {
    pub early_ply: u64,
    pub low_piece_count: u64,
    pub high_eval_default: u64,
    pub high_eval_advanced: u64,
    pub tactical_move: u64,
    pub in_check: u64,
    pub castling_move: u64,
    pub result_inconsistent_over_2500_cp: u64,
    pub bullet_default_rejected: u64,
    pub advanced_deterministic_rejected: u64,
}

/// Search-facing diagnostics for one frozen network and complete corpus.
#[derive(Clone, Debug, PartialEq)]
pub struct NetworkDiagnosticReport {
    pub games: usize,
    pub positions: u64,
    pub network_score_range_cp: (i32, i32),
    pub classical_score_range_cp: (i32, i32),
    pub teacher_score_range_cp: (i32, i32),
    pub network_vs_search: ScoreFitReport,
    pub classical_vs_search: ScoreFitReport,
    pub network_vs_classical: ScoreFitReport,
    pub score_probability_mse: f64,
    pub result_probability_mse: f64,
    pub blended_probability_mse: f64,
    pub mean_network_probability: f64,
    pub mean_score_probability: f64,
    pub mean_result_probability: f64,
    pub composition: CorpusCompositionReport,
}

/// Fail-closed input errors for network/corpus diagnostics.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NetworkDiagnosticError {
    EmptyCorpus,
    EmptyGame { index: usize },
}

impl std::fmt::Display for NetworkDiagnosticError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyCorpus => formatter.write_str("diagnostic corpus contains no games"),
            Self::EmptyGame { index } => {
                write!(
                    formatter,
                    "diagnostic game {index} contains no scored positions"
                )
            }
        }
    }
}

impl std::error::Error for NetworkDiagnosticError {}

#[derive(Clone, Copy, Debug, Default)]
struct ScoreFitAccumulator {
    samples: u64,
    reference_sum: f64,
    prediction_sum: f64,
    reference_square_sum: f64,
    prediction_square_sum: f64,
    cross_sum: f64,
    error_sum: f64,
    absolute_error_sum: f64,
    squared_error_sum: f64,
    sign_samples: u64,
    sign_agreements: u64,
}

impl ScoreFitAccumulator {
    fn push(&mut self, reference: i32, prediction: i32) {
        let reference_f64 = f64::from(reference);
        let prediction_f64 = f64::from(prediction);
        let error = prediction_f64 - reference_f64;
        self.samples += 1;
        self.reference_sum += reference_f64;
        self.prediction_sum += prediction_f64;
        self.reference_square_sum += reference_f64 * reference_f64;
        self.prediction_square_sum += prediction_f64 * prediction_f64;
        self.cross_sum += reference_f64 * prediction_f64;
        self.error_sum += error;
        self.absolute_error_sum += error.abs();
        self.squared_error_sum += error * error;
        if reference != 0 {
            self.sign_samples += 1;
            if prediction.signum() == reference.signum() {
                self.sign_agreements += 1;
            }
        }
    }

    fn report(self) -> ScoreFitReport {
        let samples = self.samples as f64;
        let covariance = samples * self.cross_sum - self.reference_sum * self.prediction_sum;
        let reference_variance =
            samples * self.reference_square_sum - self.reference_sum * self.reference_sum;
        let prediction_variance =
            samples * self.prediction_square_sum - self.prediction_sum * self.prediction_sum;
        let denominator = (reference_variance * prediction_variance).sqrt();
        let pearson_correlation = (denominator > 0.0).then_some(covariance / denominator);
        ScoreFitReport {
            samples: self.samples,
            reference_mean_cp: self.reference_sum / samples,
            prediction_mean_cp: self.prediction_sum / samples,
            mean_error_cp: self.error_sum / samples,
            mean_absolute_error_cp: self.absolute_error_sum / samples,
            root_mean_square_error_cp: (self.squared_error_sum / samples).sqrt(),
            pearson_correlation,
            sign_samples: self.sign_samples,
            sign_agreements: self.sign_agreements,
            sign_agreement_rate: if self.sign_samples == 0 {
                0.0
            } else {
                self.sign_agreements as f64 / self.sign_samples as f64
            },
        }
    }
}

/// Compare NNUE and classical SANJ directly with the stored parent-search score.
pub fn diagnose_network(
    network: &Network,
    games: &[Game],
) -> Result<NetworkDiagnosticReport, NetworkDiagnosticError> {
    if games.is_empty() {
        return Err(NetworkDiagnosticError::EmptyCorpus);
    }

    let mut positions = 0_u64;
    let mut network_range = (i32::MAX, i32::MIN);
    let mut classical_range = (i32::MAX, i32::MIN);
    let mut teacher_range = (i32::MAX, i32::MIN);
    let mut network_vs_search = ScoreFitAccumulator::default();
    let mut classical_vs_search = ScoreFitAccumulator::default();
    let mut network_vs_classical = ScoreFitAccumulator::default();
    let mut score_probability_squared_error = 0.0;
    let mut result_probability_squared_error = 0.0;
    let mut blended_probability_squared_error = 0.0;
    let mut network_probability_sum = 0.0;
    let mut score_probability_sum = 0.0;
    let mut result_probability_sum = 0.0;
    let mut composition = CorpusCompositionReport::default();

    for (game_index, game) in games.iter().enumerate() {
        if game.moves.is_empty() {
            return Err(NetworkDiagnosticError::EmptyGame { index: game_index });
        }
        let result_white = game_result_probability(game.result);
        let mut position = game.initial_position.clone();
        let mut accumulators = AccumulatorPair::refresh(&position, network);
        for scored_move in &game.moves {
            let side_to_move = position.side_to_move();
            let teacher_cp = orient_white_score(scored_move.score_cp, side_to_move);
            let result_probability = orient_white_result(result_white, side_to_move);
            let network_cp = network.evaluate(&accumulators, side_to_move);
            let classical_cp = sanj::evaluate(&position);

            network_range = extend_range(network_range, network_cp);
            classical_range = extend_range(classical_range, classical_cp);
            teacher_range = extend_range(teacher_range, teacher_cp);
            network_vs_search.push(teacher_cp, network_cp);
            classical_vs_search.push(teacher_cp, classical_cp);
            network_vs_classical.push(classical_cp, network_cp);

            let network_probability = logistic_cp(network_cp, EVAL_SCALE);
            let score_probability = logistic_cp(teacher_cp, EVAL_SCALE);
            let blended_probability = WDL_PROPORTION.mul_add(
                result_probability,
                (1.0 - WDL_PROPORTION) * score_probability,
            );
            score_probability_squared_error += (network_probability - score_probability).powi(2);
            result_probability_squared_error += (network_probability - result_probability).powi(2);
            blended_probability_squared_error +=
                (network_probability - blended_probability).powi(2);
            network_probability_sum += network_probability;
            score_probability_sum += score_probability;
            result_probability_sum += result_probability;

            update_composition(
                &mut composition,
                &position,
                scored_move.mv,
                teacher_cp,
                result_probability,
            );
            positions += 1;

            let before = position.clone();
            position.make_move(scored_move.mv);
            accumulators.update(&before, &position, network);
        }
    }

    let count = positions as f64;
    Ok(NetworkDiagnosticReport {
        games: games.len(),
        positions,
        network_score_range_cp: network_range,
        classical_score_range_cp: classical_range,
        teacher_score_range_cp: teacher_range,
        network_vs_search: network_vs_search.report(),
        classical_vs_search: classical_vs_search.report(),
        network_vs_classical: network_vs_classical.report(),
        score_probability_mse: score_probability_squared_error / count,
        result_probability_mse: result_probability_squared_error / count,
        blended_probability_mse: blended_probability_squared_error / count,
        mean_network_probability: network_probability_sum / count,
        mean_score_probability: score_probability_sum / count,
        mean_result_probability: result_probability_sum / count,
        composition,
    })
}

const fn game_result_probability(result: GameResult) -> f64 {
    match result {
        GameResult::BlackWin => 0.0,
        GameResult::Draw => 0.5,
        GameResult::WhiteWin => 1.0,
    }
}

fn orient_white_score(score: i16, side_to_move: Color) -> i32 {
    match side_to_move {
        Color::White => i32::from(score),
        Color::Black => -i32::from(score),
    }
}

fn orient_white_result(result: f64, side_to_move: Color) -> f64 {
    match side_to_move {
        Color::White => result,
        Color::Black => 1.0 - result,
    }
}

fn extend_range((minimum, maximum): (i32, i32), value: i32) -> (i32, i32) {
    (minimum.min(value), maximum.max(value))
}

fn update_composition(
    report: &mut CorpusCompositionReport,
    position: &Position,
    mv: neyrang::chess::Move,
    teacher_cp: i32,
    result_probability: f64,
) {
    let absolute_ply = 2 * (u64::from(position.fullmove_number()) - 1)
        + u64::from(position.side_to_move() == Color::Black);
    let early_ply = absolute_ply < 16;
    let low_piece_count = position.all_occupancy().count_ones() < 4;
    let high_eval_default = teacher_cp.unsigned_abs() >= 31_339;
    let high_eval_advanced = teacher_cp.unsigned_abs() >= 10_000;
    let tactical_move = mv.is_capture() || mv.is_promotion();
    let in_check = position.is_in_check(position.side_to_move());
    let castling_move = mv.is_castle();
    let result_inconsistent = if result_probability == 0.5 {
        teacher_cp.unsigned_abs() > 2_500
    } else if result_probability == 1.0 {
        teacher_cp < -2_500
    } else {
        teacher_cp > 2_500
    };

    report.early_ply += u64::from(early_ply);
    report.low_piece_count += u64::from(low_piece_count);
    report.high_eval_default += u64::from(high_eval_default);
    report.high_eval_advanced += u64::from(high_eval_advanced);
    report.tactical_move += u64::from(tactical_move);
    report.in_check += u64::from(in_check);
    report.castling_move += u64::from(castling_move);
    report.result_inconsistent_over_2500_cp += u64::from(result_inconsistent);
    report.bullet_default_rejected +=
        u64::from(early_ply || low_piece_count || high_eval_default || tactical_move || in_check);
    report.advanced_deterministic_rejected += u64::from(
        early_ply
            || low_piece_count
            || high_eval_advanced
            || tactical_move
            || in_check
            || castling_move
            || result_inconsistent,
    );
}

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

/// Four-way paired sign-agreement transitions on one frozen sample slice.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SignTransitionReport {
    pub samples: u64,
    pub both_correct: u64,
    pub baseline_only_correct: u64,
    pub candidate_only_correct: u64,
    pub neither_correct: u64,
    pub baseline_agreements: u64,
    pub candidate_agreements: u64,
    pub baseline_agreement_rate: f64,
    pub candidate_agreement_rate: f64,
    pub candidate_minus_baseline_rate: f64,
}

/// Stable key plus transitions for one mutually exclusive or contextual slice.
#[derive(Clone, Debug, PartialEq)]
pub struct NamedSignSliceReport {
    pub key: String,
    pub transitions: SignTransitionReport,
}

/// Paired sign-agreement diagnosis with position- and game-equal weighting.
#[derive(Clone, Debug, PartialEq)]
pub struct PairedSignDiagnosticReport {
    pub games: usize,
    pub signed_games: usize,
    pub positions: u64,
    pub overall: SignTransitionReport,
    pub score_magnitude_bands: Vec<NamedSignSliceReport>,
    pub stm_king_buckets: Vec<NamedSignSliceReport>,
    pub ntm_king_buckets: Vec<NamedSignSliceReport>,
    pub king_bucket_pairs: Vec<NamedSignSliceReport>,
    pub piece_count_bands: Vec<NamedSignSliceReport>,
    pub side_to_move: Vec<NamedSignSliceReport>,
    pub contexts: Vec<NamedSignSliceReport>,
    pub game_equal_mean_delta_sign_agreement: f64,
    pub improved_games: usize,
    pub tied_games: usize,
    pub regressed_games: usize,
    pub bootstrap_lower_2_5_percentile: f64,
    pub bootstrap_upper_97_5_percentile: f64,
    pub bootstrap_seed: u64,
    pub bootstrap_replicates: usize,
}

#[derive(Clone, Copy, Debug, Default)]
struct SignTransitionAccumulator {
    samples: u64,
    both_correct: u64,
    baseline_only_correct: u64,
    candidate_only_correct: u64,
    neither_correct: u64,
}

impl SignTransitionAccumulator {
    fn push(&mut self, baseline_correct: bool, candidate_correct: bool) {
        self.samples += 1;
        match (baseline_correct, candidate_correct) {
            (true, true) => self.both_correct += 1,
            (true, false) => self.baseline_only_correct += 1,
            (false, true) => self.candidate_only_correct += 1,
            (false, false) => self.neither_correct += 1,
        }
    }

    fn report(self) -> SignTransitionReport {
        let baseline_agreements = self.both_correct + self.baseline_only_correct;
        let candidate_agreements = self.both_correct + self.candidate_only_correct;
        let samples = self.samples as f64;
        let baseline_agreement_rate = baseline_agreements as f64 / samples;
        let candidate_agreement_rate = candidate_agreements as f64 / samples;
        SignTransitionReport {
            samples: self.samples,
            both_correct: self.both_correct,
            baseline_only_correct: self.baseline_only_correct,
            candidate_only_correct: self.candidate_only_correct,
            neither_correct: self.neither_correct,
            baseline_agreements,
            candidate_agreements,
            baseline_agreement_rate,
            candidate_agreement_rate,
            candidate_minus_baseline_rate: candidate_agreement_rate - baseline_agreement_rate,
        }
    }
}

/// Fail-closed input errors for paired model diagnostics.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PairedComparisonError {
    EmptyCorpus,
    EmptyGame { index: usize },
    ZeroBootstrapReplicates,
    ParameterMismatch,
    NoSignedPositions,
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
            Self::NoSignedPositions => {
                formatter.write_str("comparison corpus contains no non-zero teacher scores")
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

/// Diagnose exactly which paired samples gain or lose teacher-score sign agreement.
pub fn diagnose_sign_disagreements(
    baseline: &Network,
    candidate: &Network,
    games: &[Game],
    bootstrap_seed: u64,
    bootstrap_replicates: usize,
) -> Result<PairedSignDiagnosticReport, PairedComparisonError> {
    if games.is_empty() {
        return Err(PairedComparisonError::EmptyCorpus);
    }
    if bootstrap_replicates == 0 {
        return Err(PairedComparisonError::ZeroBootstrapReplicates);
    }
    if baseline.parameters() != candidate.parameters() {
        return Err(PairedComparisonError::ParameterMismatch);
    }

    let mut positions = 0_u64;
    let mut overall = SignTransitionAccumulator::default();
    let mut score_magnitude_bands = [SignTransitionAccumulator::default(); 9];
    let mut stm_king_buckets = [SignTransitionAccumulator::default(); 3];
    let mut ntm_king_buckets = [SignTransitionAccumulator::default(); 3];
    let mut king_bucket_pairs = [SignTransitionAccumulator::default(); 9];
    let mut piece_count_bands = [SignTransitionAccumulator::default(); 4];
    let mut side_to_move_slices = [SignTransitionAccumulator::default(); 2];
    let mut contexts = [SignTransitionAccumulator::default(); 8];
    let mut game_deltas = Vec::with_capacity(games.len());
    for (game_index, game) in games.iter().enumerate() {
        if game.moves.is_empty() {
            return Err(PairedComparisonError::EmptyGame { index: game_index });
        }
        let mut position = game.initial_position.clone();
        let mut baseline_accumulators = AccumulatorPair::refresh(&position, baseline);
        let mut candidate_accumulators = AccumulatorPair::refresh(&position, candidate);
        let mut game_baseline_agreements = 0_u64;
        let mut game_candidate_agreements = 0_u64;
        let mut game_sign_samples = 0_u64;

        for scored_move in &game.moves {
            let side_to_move = position.side_to_move();
            let teacher_cp = orient_white_score(scored_move.score_cp, side_to_move);
            if teacher_cp != 0 {
                let baseline_cp = baseline.evaluate(&baseline_accumulators, side_to_move);
                let candidate_cp = candidate.evaluate(&candidate_accumulators, side_to_move);
                let baseline_correct = baseline_cp.signum() == teacher_cp.signum();
                let candidate_correct = candidate_cp.signum() == teacher_cp.signum();
                overall.push(baseline_correct, candidate_correct);
                score_magnitude_bands[score_magnitude_band(teacher_cp.unsigned_abs())]
                    .push(baseline_correct, candidate_correct);
                let stm_bucket = usize::from(king_bucket_mirrored_3(&position, side_to_move));
                let ntm_bucket =
                    usize::from(king_bucket_mirrored_3(&position, side_to_move.opposite()));
                stm_king_buckets[stm_bucket].push(baseline_correct, candidate_correct);
                ntm_king_buckets[ntm_bucket].push(baseline_correct, candidate_correct);
                king_bucket_pairs[stm_bucket * 3 + ntm_bucket]
                    .push(baseline_correct, candidate_correct);
                let piece_count = position.all_occupancy().count_ones();
                piece_count_bands[piece_count_band(piece_count)]
                    .push(baseline_correct, candidate_correct);
                side_to_move_slices[side_to_move.index()].push(baseline_correct, candidate_correct);
                let absolute_ply = 2 * (u64::from(position.fullmove_number()) - 1)
                    + u64::from(side_to_move == Color::Black);
                let early_ply = absolute_ply < 16;
                let tactical_move = scored_move.mv.is_capture() || scored_move.mv.is_promotion();
                let in_check = position.is_in_check(side_to_move);
                let castling_move = scored_move.mv.is_castle();
                contexts[usize::from(!early_ply)].push(baseline_correct, candidate_correct);
                contexts[2 + usize::from(!tactical_move)].push(baseline_correct, candidate_correct);
                contexts[4 + usize::from(!in_check)].push(baseline_correct, candidate_correct);
                contexts[6 + usize::from(!castling_move)].push(baseline_correct, candidate_correct);
                game_baseline_agreements += u64::from(baseline_correct);
                game_candidate_agreements += u64::from(candidate_correct);
                game_sign_samples += 1;
            }
            positions += 1;

            let before = position.clone();
            position.make_move(scored_move.mv);
            baseline_accumulators.update(&before, &position, baseline);
            candidate_accumulators.update(&before, &position, candidate);
        }

        if game_sign_samples > 0 {
            game_deltas.push(
                (game_candidate_agreements as f64 - game_baseline_agreements as f64)
                    / game_sign_samples as f64,
            );
        }
    }

    if game_deltas.is_empty() {
        return Err(PairedComparisonError::NoSignedPositions);
    }
    let game_equal_mean_delta_sign_agreement =
        game_deltas.iter().sum::<f64>() / game_deltas.len() as f64;
    let improved_games = game_deltas.iter().filter(|&&delta| delta > 0.0).count();
    let tied_games = game_deltas.iter().filter(|&&delta| delta == 0.0).count();
    let regressed_games = game_deltas.len() - improved_games - tied_games;
    let (bootstrap_lower_2_5_percentile, bootstrap_upper_97_5_percentile) =
        bootstrap_interval(&game_deltas, bootstrap_seed, bootstrap_replicates);

    Ok(PairedSignDiagnosticReport {
        games: games.len(),
        signed_games: game_deltas.len(),
        positions,
        overall: overall.report(),
        score_magnitude_bands: named_sign_slices(
            &[
                "1-25",
                "26-50",
                "51-100",
                "101-200",
                "201-400",
                "401-800",
                "801-1600",
                "1601-2500",
                "2501+",
            ],
            score_magnitude_bands,
        ),
        stm_king_buckets: named_sign_slices(&["0", "1", "2"], stm_king_buckets),
        ntm_king_buckets: named_sign_slices(&["0", "1", "2"], ntm_king_buckets),
        king_bucket_pairs: named_sign_slices(
            &[
                "0:0", "0:1", "0:2", "1:0", "1:1", "1:2", "2:0", "2:1", "2:2",
            ],
            king_bucket_pairs,
        ),
        piece_count_bands: named_sign_slices(&["2-7", "8-15", "16-23", "24-32"], piece_count_bands),
        side_to_move: named_sign_slices(&["white", "black"], side_to_move_slices),
        contexts: named_sign_slices(
            &[
                "early_ply",
                "not_early_ply",
                "tactical_move",
                "not_tactical_move",
                "in_check",
                "not_in_check",
                "castling_move",
                "not_castling_move",
            ],
            contexts,
        ),
        game_equal_mean_delta_sign_agreement,
        improved_games,
        tied_games,
        regressed_games,
        bootstrap_lower_2_5_percentile,
        bootstrap_upper_97_5_percentile,
        bootstrap_seed,
        bootstrap_replicates,
    })
}

fn score_magnitude_band(absolute_score_cp: u32) -> usize {
    match absolute_score_cp {
        1..=25 => 0,
        26..=50 => 1,
        51..=100 => 2,
        101..=200 => 3,
        201..=400 => 4,
        401..=800 => 5,
        801..=1_600 => 6,
        1_601..=2_500 => 7,
        _ => 8,
    }
}

fn piece_count_band(piece_count: u32) -> usize {
    match piece_count {
        0..=7 => 0,
        8..=15 => 1,
        16..=23 => 2,
        _ => 3,
    }
}

fn named_sign_slices<const N: usize>(
    keys: &[&str; N],
    accumulators: [SignTransitionAccumulator; N],
) -> Vec<NamedSignSliceReport> {
    keys.iter()
        .zip(accumulators)
        .map(|(key, transitions)| NamedSignSliceReport {
            key: (*key).to_owned(),
            transitions: if transitions.samples == 0 {
                SignTransitionReport::default()
            } else {
                transitions.report()
            },
        })
        .collect()
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
