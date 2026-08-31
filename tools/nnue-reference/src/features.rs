use neyrang::chess::{Color, PieceType, Position, Square};

/// Two colors, six piece kinds and 64 squares.
pub const INPUT_FEATURES: usize = 2 * 6 * 64;

/// Map a piece-square pair into the friendly/enemy plane for one perspective.
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
    let oriented_square = match perspective {
        Color::White => square.index(),
        Color::Black => square.index() ^ 56,
    };
    (relative_color * 6 + piece_type.index()) * 64 + oriented_square
}

/// Return all active Chess768 features for a board and one perspective.
#[must_use]
pub fn active_features(position: &Position, perspective: Color) -> Vec<usize> {
    let mut features = Vec::with_capacity(32);
    for index in 0_u8..64 {
        let square = Square::from_index(index).expect("board index is in range");
        if let Some((color, piece_type)) = position.piece_at(square) {
            features.push(feature_index(color, piece_type, square, perspective));
        }
    }
    features
}
