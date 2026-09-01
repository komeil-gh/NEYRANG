use neyrang::chess::Color;
use neyrang_nnue_reference::{blended_target, logistic_cp};

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
