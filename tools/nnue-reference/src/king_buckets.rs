use neyrang::chess::{Color, PieceType};
use neyrang_nnue_data::Game;

/// King-square exposure for the two perspective inputs used by the trainer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KingBucketReport {
    pub games: usize,
    pub positions: u64,
    pub perspective_samples: u64,
    pub oriented_king_squares: [u64; 64],
    pub horizontally_mirrored_king_squares: [u64; 32],
}

/// Fail-closed errors for king-bucket corpus analysis.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KingBucketError {
    EmptyCorpus,
    EmptyGame { index: usize },
}

impl std::fmt::Display for KingBucketError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyCorpus => formatter.write_str("king-bucket corpus contains no games"),
            Self::EmptyGame { index } => {
                write!(
                    formatter,
                    "king-bucket game {index} contains no scored positions"
                )
            }
        }
    }
}

impl std::error::Error for KingBucketError {}

/// Count both oriented king perspectives for every scored parent position.
pub fn analyze_king_buckets(games: &[Game]) -> Result<KingBucketReport, KingBucketError> {
    if games.is_empty() {
        return Err(KingBucketError::EmptyCorpus);
    }

    let mut positions = 0_u64;
    let mut oriented_king_squares = [0_u64; 64];
    let mut horizontally_mirrored_king_squares = [0_u64; 32];
    for (game_index, game) in games.iter().enumerate() {
        if game.moves.is_empty() {
            return Err(KingBucketError::EmptyGame { index: game_index });
        }
        let mut position = game.initial_position.clone();
        for scored_move in &game.moves {
            for perspective in [Color::White, Color::Black] {
                let king = position.pieces(perspective, PieceType::King);
                debug_assert_eq!(king.count_ones(), 1);
                let square = king.trailing_zeros() as usize;
                let oriented = match perspective {
                    Color::White => square,
                    Color::Black => square ^ 56,
                };
                let rank = oriented / 8;
                let file = oriented % 8;
                let mirrored_file = file.min(7 - file);
                oriented_king_squares[oriented] += 1;
                horizontally_mirrored_king_squares[rank * 4 + mirrored_file] += 1;
            }
            positions += 1;
            position.make_move(scored_move.mv);
        }
    }

    Ok(KingBucketReport {
        games: games.len(),
        positions,
        perspective_samples: positions * 2,
        oriented_king_squares,
        horizontally_mirrored_king_squares,
    })
}
