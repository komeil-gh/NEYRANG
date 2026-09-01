use neyrang::chess::Color;

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
