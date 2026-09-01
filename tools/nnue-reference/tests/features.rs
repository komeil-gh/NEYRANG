use neyrang::chess::{Color, PieceType, Position, Square};
use neyrang_nnue_reference::{
    FeatureSet, INPUT_FEATURES, INPUT_FEATURES_KING_BUCKETS_MIRRORED_3, active_features,
    active_features_for, feature_index, feature_index_for,
};

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

#[test]
fn three_bank_mapping_uses_own_king_bucket_and_horizontal_mirror() {
    let home = Position::startpos();
    assert_eq!(INPUT_FEATURES_KING_BUCKETS_MIRRORED_3, 3 * INPUT_FEATURES);

    assert_eq!(
        feature_index_for(
            &home,
            Color::White,
            PieceType::Pawn,
            Square::A2,
            Color::White,
            FeatureSet::Chess768KingBucketsMirrored3,
        ),
        15
    );
    assert_eq!(
        feature_index_for(
            &home,
            Color::Black,
            PieceType::Pawn,
            Square::A7,
            Color::Black,
            FeatureSet::Chess768KingBucketsMirrored3,
        ),
        15
    );

    let castled =
        Position::from_fen("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQ1RK1 w kq - 0 1").unwrap();
    assert_eq!(
        feature_index_for(
            &castled,
            Color::White,
            PieceType::Pawn,
            Square::A2,
            Color::White,
            FeatureSet::Chess768KingBucketsMirrored3,
        ),
        INPUT_FEATURES + 15
    );
}

#[test]
fn three_bank_active_features_are_unique_and_in_range() {
    let position = Position::startpos();
    for perspective in [Color::White, Color::Black] {
        let mut features = active_features_for(
            &position,
            perspective,
            FeatureSet::Chess768KingBucketsMirrored3,
        );
        assert_eq!(features.len(), 32);
        assert!(
            features
                .iter()
                .all(|&feature| feature < INPUT_FEATURES_KING_BUCKETS_MIRRORED_3)
        );
        features.sort_unstable();
        features.dedup();
        assert_eq!(features.len(), 32);
    }
}
