use neyrang::chess::{Color, PieceType, Position, Square};
use neyrang_nnue_reference::{INPUT_FEATURES, active_features, feature_index};

#[test]
fn chess768_maps_piece_planes_from_each_perspective() {
    assert_eq!(INPUT_FEATURES, 768);

    assert_eq!(
        feature_index(Color::White, PieceType::Pawn, Square::A2, Color::White),
        8
    );
    assert_eq!(
        feature_index(Color::Black, PieceType::Pawn, Square::A7, Color::White),
        6 * 64 + 48
    );
    assert_eq!(
        feature_index(Color::Black, PieceType::Pawn, Square::A7, Color::Black),
        8
    );
    assert_eq!(
        feature_index(Color::White, PieceType::Pawn, Square::A2, Color::Black),
        6 * 64 + 48
    );
}

#[test]
fn start_position_has_32_unique_features_per_perspective() {
    let position = Position::startpos();

    for perspective in [Color::White, Color::Black] {
        let mut features = active_features(&position, perspective);
        assert_eq!(features.len(), 32);
        features.sort_unstable();
        features.dedup();
        assert_eq!(features.len(), 32);
        assert!(features.iter().all(|&feature| feature < INPUT_FEATURES));
    }
}
