use neyrang::chess::{Color, PieceType, Position, Square};

/// Two colors, six piece kinds and 64 squares.
pub const INPUT_FEATURES: usize = 2 * 6 * 64;
/// Three horizontally mirrored own-king banks of Chess768 features.
pub const INPUT_FEATURES_KING_BUCKETS_MIRRORED_3: usize = 3 * INPUT_FEATURES;

/// Supported sparse input mappings for the scalar reference.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FeatureSet {
    Chess768,
    Chess768KingBucketsMirrored3,
}

impl FeatureSet {
    #[must_use]
    pub const fn input_features(self) -> usize {
        match self {
            Self::Chess768 => INPUT_FEATURES,
            Self::Chess768KingBucketsMirrored3 => INPUT_FEATURES_KING_BUCKETS_MIRRORED_3,
        }
    }
}

/// Rank-major layout after folding files `a..h` into `a..d`.
pub const KING_BUCKET_LAYOUT_MIRRORED_3: [u8; 32] = [
    1, 1, 1, 0, // home rank: d/e king is uncastled; other files are sheltered
    1, 1, 1, 1, // second rank
    2, 2, 2, 2, // active king ranks
    2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2,
];

/// Map a piece-square pair into the friendly/enemy Chess768 plane.
#[must_use]
pub const fn feature_index(
    piece_color: Color,
    piece_type: PieceType,
    square: Square,
    perspective: Color,
) -> usize {
    let relative_color = if piece_color as u8 == perspective as u8 {
        0
    } else {
        1
    };
    let oriented_square = orient_square(square.index(), perspective);
    (relative_color * 6 + piece_type.index()) * 64 + oriented_square
}

/// Map a piece-square pair under a selected feature-set contract.
#[must_use]
pub fn feature_index_for(
    position: &Position,
    piece_color: Color,
    piece_type: PieceType,
    square: Square,
    perspective: Color,
    feature_set: FeatureSet,
) -> usize {
    let base = feature_index(piece_color, piece_type, square, perspective);
    match feature_set {
        FeatureSet::Chess768 => base,
        FeatureSet::Chess768KingBucketsMirrored3 => {
            let king_square = position
                .pieces(perspective, PieceType::King)
                .trailing_zeros() as usize;
            let oriented_king = orient_square(king_square, perspective);
            let rank = oriented_king / 8;
            let file = oriented_king % 8;
            let mirrored_file = file.min(7 - file);
            let bucket = usize::from(KING_BUCKET_LAYOUT_MIRRORED_3[rank * 4 + mirrored_file]);
            let horizontal_flip = if file > 3 { 7 } else { 0 };
            bucket * INPUT_FEATURES + (base ^ horizontal_flip)
        }
    }
}

/// Return all active Chess768 features for a board and one perspective.
#[must_use]
pub fn active_features(position: &Position, perspective: Color) -> Vec<usize> {
    active_features_for(position, perspective, FeatureSet::Chess768)
}

/// Return all active features for a board, perspective and mapping contract.
#[must_use]
pub fn active_features_for(
    position: &Position,
    perspective: Color,
    feature_set: FeatureSet,
) -> Vec<usize> {
    let mut features = Vec::with_capacity(32);
    for index in 0_u8..64 {
        let square = Square::from_index(index).expect("board index is in range");
        if let Some((color, piece_type)) = position.piece_at(square) {
            features.push(feature_index_for(
                position,
                color,
                piece_type,
                square,
                perspective,
                feature_set,
            ));
        }
    }
    features
}

const fn orient_square(square: usize, perspective: Color) -> usize {
    match perspective {
        Color::White => square,
        Color::Black => square ^ 56,
    }
}
