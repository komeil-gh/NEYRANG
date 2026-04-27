use super::{Move, Position};

/// Count legal leaf nodes at an exact depth.
pub fn perft(position: &mut Position, depth: u8) -> u64 {
    if depth == 0 {
        return 1;
    }
    let moves = position.legal_moves();
    if depth == 1 {
        return moves.len() as u64;
    }

    let mut nodes = 0_u64;
    for &mv in moves.iter() {
        let undo = position.make_move(mv);
        nodes += perft(position, depth - 1);
        position.unmake_move(mv, undo);
    }
    nodes
}

/// Per-root-move counts used to isolate move-generation defects.
pub fn divide(position: &mut Position, depth: u8) -> Vec<(Move, u64)> {
    if depth == 0 {
        return Vec::new();
    }
    let moves = position.legal_moves();
    let mut result = Vec::with_capacity(moves.len());
    for &mv in moves.iter() {
        let undo = position.make_move(mv);
        let nodes = perft(position, depth - 1);
        position.unmake_move(mv, undo);
        result.push((mv, nodes));
    }
    result
}
