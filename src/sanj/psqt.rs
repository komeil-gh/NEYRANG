use crate::chess::{Color, PieceType, Square};

/// Original, compact PSQT formulae keep the first evaluator inspectable and
/// avoid embedding tables whose provenance would be difficult to audit.
pub(crate) const fn middlegame(kind: PieceType, square: Square, color: Color) -> i32 {
    let file = square.file() as i32;
    let rank = relative_rank(square, color);
    let file_edge = edge_distance(file);
    let rank_edge = edge_distance(rank);
    match kind {
        PieceType::Pawn => rank * 7 + file_edge * 2,
        PieceType::Knight => (file_edge + rank_edge) * 9 - 24,
        PieceType::Bishop => (file_edge + rank_edge) * 5 - 12,
        PieceType::Rook => rank * 2 + file_edge,
        PieceType::Queen => (file_edge + rank_edge) * 2 - 5,
        PieceType::King => -rank * 9 - file_edge * 3,
    }
}

pub(crate) const fn endgame(kind: PieceType, square: Square, color: Color) -> i32 {
    let file = square.file() as i32;
    let rank = relative_rank(square, color);
    let file_edge = edge_distance(file);
    let rank_edge = edge_distance(rank);
    match kind {
        PieceType::Pawn => rank * 12 + file_edge,
        PieceType::Knight => (file_edge + rank_edge) * 7 - 18,
        PieceType::Bishop => (file_edge + rank_edge) * 4 - 10,
        PieceType::Rook => rank * 3,
        PieceType::Queen => file_edge + rank_edge,
        PieceType::King => (file_edge + rank_edge) * 8 - 20,
    }
}

const fn relative_rank(square: Square, color: Color) -> i32 {
    match color {
        Color::White => square.rank() as i32,
        Color::Black => 7 - square.rank() as i32,
    }
}

const fn edge_distance(coordinate: i32) -> i32 {
    let from_low = coordinate;
    let from_high = 7 - coordinate;
    if from_low < from_high {
        from_low
    } else {
        from_high
    }
}
