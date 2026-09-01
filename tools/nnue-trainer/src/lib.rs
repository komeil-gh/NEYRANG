//! Fixed, auditable plan calculations for the first NEYRANG NNUE epoch.

use std::{
    fmt,
    io::{BufRead, BufReader},
    path::PathBuf,
};

use bullet::game::formats::{bulletformat::ChessBoard, viriformat::dataformat::Game};
use bullet_trainer::reader::DataReader;

pub const POSITION_SHUFFLE_SEED: u64 = 2_026_090_111;
pub const ACTIVATION_QUANT: i16 = 511;
pub const OUTPUT_QUANT: i16 = 768;
pub const OUTPUT_BIAS_QUANT: i32 = ACTIVATION_QUANT as i32 * OUTPUT_QUANT as i32;

/// The exact number of complete batches consumed by one bounded epoch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EpochPlan {
    pub positions: usize,
    pub batch_size: usize,
    pub batches: usize,
}

/// Invalid epoch geometry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlanError {
    ZeroPositions,
    ZeroBatchSize,
    IncompleteBatch { positions: usize, batch_size: usize },
}

impl fmt::Display for PlanError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroPositions => formatter.write_str("epoch positions must be positive"),
            Self::ZeroBatchSize => formatter.write_str("batch size must be positive"),
            Self::IncompleteBatch {
                positions,
                batch_size,
            } => write!(
                formatter,
                "{positions} positions are not exactly divisible by batch size {batch_size}"
            ),
        }
    }
}

impl std::error::Error for PlanError {}

impl EpochPlan {
    pub fn new(positions: usize, batch_size: usize) -> Result<Self, PlanError> {
        if positions == 0 {
            return Err(PlanError::ZeroPositions);
        }
        if batch_size == 0 {
            return Err(PlanError::ZeroBatchSize);
        }
        if !positions.is_multiple_of(batch_size) {
            return Err(PlanError::IncompleteBatch {
                positions,
                batch_size,
            });
        }
        Ok(Self {
            positions,
            batch_size,
            batches: positions / batch_size,
        })
    }
}

/// Sequential Viriformat reader for already deterministically game-shuffled corpora.
///
/// Bullet's builtin loader intentionally reshuffles with a wall-clock seed. NEYRANG's
/// corpus order is already frozen in its manifest, so this reader preserves it and
/// makes repeated single-device epochs byte-reproducible.
#[derive(Clone, Debug)]
pub struct DeterministicViriLoader {
    path: PathBuf,
    chunk_positions: usize,
}

impl DeterministicViriLoader {
    pub fn new(path: impl Into<PathBuf>, chunk_positions: usize) -> Result<Self, PlanError> {
        if chunk_positions == 0 {
            return Err(PlanError::ZeroBatchSize);
        }
        Ok(Self {
            path: path.into(),
            chunk_positions,
        })
    }
}

impl DataReader<ChessBoard> for DeterministicViriLoader {
    fn read_chunks<F: FnMut(&[ChessBoard]) -> bool>(&self, skip_count: usize, mut callback: F) {
        let mut skip_remaining = skip_count;
        let mut chunk_index = 0_u64;
        loop {
            let file = std::fs::File::open(&self.path).expect("open audited Viriformat corpus");
            let mut reader = BufReader::new(file);
            let mut move_buffer = Vec::new();
            let mut positions = Vec::with_capacity(self.chunk_positions);

            loop {
                if reader
                    .fill_buf()
                    .expect("read audited Viriformat corpus")
                    .is_empty()
                {
                    break;
                }
                let game = Game::deserialise_from(&mut reader, move_buffer)
                    .expect("decode audited Viriformat game");
                game.splat_to_bulletformat_with_filter_callback(
                    |position| {
                        if skip_remaining > 0 {
                            skip_remaining -= 1;
                        } else {
                            positions.push(position);
                        }
                        Ok(())
                    },
                    |_, _, _, _, _| false,
                )
                .expect("convert audited Viriformat game");
                move_buffer = game.into_move_buffer();

                if positions.len() >= self.chunk_positions {
                    deterministic_shuffle(&mut positions, POSITION_SHUFFLE_SEED ^ chunk_index);
                    chunk_index = chunk_index.wrapping_add(1);
                    if callback(&positions) {
                        return;
                    }
                    positions.clear();
                }
            }

            if !positions.is_empty() {
                deterministic_shuffle(&mut positions, POSITION_SHUFFLE_SEED ^ chunk_index);
                chunk_index = chunk_index.wrapping_add(1);
                if callback(&positions) {
                    return;
                }
            }
        }
    }
}

fn deterministic_shuffle<T>(values: &mut [T], seed: u64) {
    let mut state = seed;
    for upper in (1..values.len()).rev() {
        state = state.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut random = state;
        random = (random ^ (random >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        random = (random ^ (random >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        random ^= random >> 31;
        values.swap(upper, random as usize % (upper + 1));
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ACTIVATION_QUANT, EpochPlan, OUTPUT_BIAS_QUANT, OUTPUT_QUANT, PlanError,
        deterministic_shuffle,
    };

    #[test]
    fn n1d_quantization_contract_matches_the_passing_parity_candidate() {
        assert_eq!(ACTIVATION_QUANT, 511);
        assert_eq!(OUTPUT_QUANT, 768);
        assert_eq!(OUTPUT_BIAS_QUANT, 511 * 768);
    }

    #[test]
    fn million_gate_geometry_is_one_exact_pass() {
        assert_eq!(
            EpochPlan::new(980_256, 10_211),
            Ok(EpochPlan {
                positions: 980_256,
                batch_size: 10_211,
                batches: 96,
            })
        );
    }

    #[test]
    fn incomplete_last_batch_is_rejected() {
        assert_eq!(
            EpochPlan::new(1_000_000, 16_384),
            Err(PlanError::IncompleteBatch {
                positions: 1_000_000,
                batch_size: 16_384,
            })
        );
    }

    #[test]
    fn position_shuffle_is_seeded_and_reproducible() {
        let source: Vec<u32> = (0..128).collect();
        let mut first = source.clone();
        let mut second = source.clone();
        let mut another_seed = source.clone();

        deterministic_shuffle(&mut first, 20_260_901);
        deterministic_shuffle(&mut second, 20_260_901);
        deterministic_shuffle(&mut another_seed, 20_260_902);

        assert_eq!(first, second);
        assert_ne!(first, source);
        assert_ne!(first, another_seed);
        first.sort_unstable();
        assert_eq!(first, source);
    }
}
