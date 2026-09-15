//! Fixed, auditable plan calculations for the first NEYRANG NNUE epoch.

use std::{
    fmt,
    io::{BufRead, BufReader},
    path::PathBuf,
};

use bullet::game::formats::{bulletformat::ChessBoard, viriformat::dataformat::Game};
use bullet_trainer::reader::DataReader;

mod teacher_text;
pub use teacher_text::TeacherTextLoader;

pub const POSITION_SHUFFLE_SEED: u64 = 2_026_090_111;
pub const ACTIVATION_QUANT: i16 = 511;
pub const OUTPUT_QUANT: i16 = 768;
pub const OUTPUT_BIAS_QUANT: i32 = ACTIVATION_QUANT as i32 * OUTPUT_QUANT as i32;

/// Invalid checkpoint tensors for a factorised raw export.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FactorisedRawError {
    InvalidGeometry,
    MissingTensor(&'static str),
    DuplicateTensor(&'static str),
    ShapeMismatch {
        tensor: &'static str,
        expected: usize,
        actual: usize,
    },
    NonFiniteTensor(&'static str),
}

impl fmt::Display for FactorisedRawError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidGeometry => {
                formatter.write_str("factorised raw geometry must be positive")
            }
            Self::MissingTensor(tensor) => write!(formatter, "missing checkpoint tensor {tensor}"),
            Self::DuplicateTensor(tensor) => {
                write!(formatter, "duplicate checkpoint tensor {tensor}")
            }
            Self::ShapeMismatch {
                tensor,
                expected,
                actual,
            } => write!(
                formatter,
                "checkpoint tensor {tensor} has {actual} values; expected {expected}"
            ),
            Self::NonFiniteTensor(tensor) => {
                write!(
                    formatter,
                    "checkpoint tensor {tensor} contains non-finite values"
                )
            }
        }
    }
}

impl std::error::Error for FactorisedRawError {}

/// Merge a shared base feature transformer into every input bank and emit the
/// float tensor order consumed by the NEYRANG scalar reference.
pub fn merge_factorised_raw_tensors(
    tensors: &[(String, Vec<f32>)],
    buckets: usize,
    hidden_size: usize,
    base_features: usize,
) -> Result<Vec<f32>, FactorisedRawError> {
    merge_factorised_raw_tensors_with_output_heads(tensors, buckets, hidden_size, base_features, 1)
}

/// Merge input factorisation and transpose Bullet's output matrix into
/// contiguous material-head rows for scalar inference.
pub fn merge_factorised_raw_tensors_with_output_heads(
    tensors: &[(String, Vec<f32>)],
    buckets: usize,
    hidden_size: usize,
    base_features: usize,
    output_heads: usize,
) -> Result<Vec<f32>, FactorisedRawError> {
    let output_inputs = hidden_size
        .checked_mul(2)
        .ok_or(FactorisedRawError::InvalidGeometry)?;
    merge_factorised_raw_tensors_with_output_shape(
        tensors,
        buckets,
        hidden_size,
        base_features,
        output_inputs,
        output_heads,
    )
}

/// Merge input factorisation with an explicitly shaped output matrix.
pub fn merge_factorised_raw_tensors_with_output_shape(
    tensors: &[(String, Vec<f32>)],
    buckets: usize,
    hidden_size: usize,
    base_features: usize,
    output_inputs: usize,
    output_heads: usize,
) -> Result<Vec<f32>, FactorisedRawError> {
    if buckets == 0
        || hidden_size == 0
        || base_features == 0
        || output_inputs == 0
        || output_heads == 0
    {
        return Err(FactorisedRawError::InvalidGeometry);
    }

    let get = |name: &'static str, expected: usize| -> Result<&[f32], FactorisedRawError> {
        let mut matches = tensors
            .iter()
            .filter(|(tensor_name, _)| tensor_name == name);
        let values = matches
            .next()
            .ok_or(FactorisedRawError::MissingTensor(name))?
            .1
            .as_slice();
        if matches.next().is_some() {
            return Err(FactorisedRawError::DuplicateTensor(name));
        }
        if values.len() != expected {
            return Err(FactorisedRawError::ShapeMismatch {
                tensor: name,
                expected,
                actual: values.len(),
            });
        }
        if values.iter().any(|value| !value.is_finite()) {
            return Err(FactorisedRawError::NonFiniteTensor(name));
        }
        Ok(values)
    };

    let bank_values = base_features
        .checked_mul(hidden_size)
        .ok_or(FactorisedRawError::InvalidGeometry)?;
    let bucket_values = bank_values
        .checked_mul(buckets)
        .ok_or(FactorisedRawError::InvalidGeometry)?;
    let l0w = get("l0w", bucket_values)?;
    let l0f = get("l0f", bank_values)?;
    let l0b = get("l0b", hidden_size)?;
    let l1w = get("l1w", output_inputs * output_heads)?;
    let l1b = get("l1b", output_heads)?;

    let mut merged = Vec::with_capacity(
        bucket_values + hidden_size + output_inputs * output_heads + output_heads,
    );
    for bank in l0w.chunks_exact(bank_values) {
        merged.extend(
            bank.iter()
                .zip(l0f)
                .map(|(specific, shared)| specific + shared),
        );
    }
    merged.extend_from_slice(l0b);
    for head in 0..output_heads {
        for input in 0..output_inputs {
            merged.push(l1w[input * output_heads + head]);
        }
    }
    merged.extend_from_slice(l1b);
    Ok(merged)
}

/// Stable position-filter choices admitted by the NEYRANG training contract.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum PositionFilter {
    #[default]
    None,
    BulletDefault,
}

impl PositionFilter {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::BulletDefault => "bullet-default",
        }
    }

    pub const fn rejects(self, facts: FilterFacts) -> bool {
        match self {
            Self::None => false,
            Self::BulletDefault => {
                facts.ply < 16
                    || facts.pieces < 4
                    || facts.eval_cp.unsigned_abs() >= 31_339
                    || facts.tactical
                    || facts.in_check
            }
        }
    }
}

impl std::str::FromStr for PositionFilter {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "none" => Ok(Self::None),
            "bullet-default" => Ok(Self::BulletDefault),
            _ => Err(format!(
                "unknown position filter {value:?}; expected none or bullet-default"
            )),
        }
    }
}

/// Minimal facts needed to reproduce Viriformat 2.0.1's deterministic default filter.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FilterFacts {
    pub ply: usize,
    pub pieces: u32,
    pub eval_cp: i32,
    pub tactical: bool,
    pub in_check: bool,
}

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
    filter: PositionFilter,
}

impl DeterministicViriLoader {
    pub fn new(path: impl Into<PathBuf>, chunk_positions: usize) -> Result<Self, PlanError> {
        if chunk_positions == 0 {
            return Err(PlanError::ZeroBatchSize);
        }
        Ok(Self {
            path: path.into(),
            chunk_positions,
            filter: PositionFilter::None,
        })
    }

    pub const fn with_filter(mut self, filter: PositionFilter) -> Self {
        self.filter = filter;
        self
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
                    |mv, eval, board, _, _| {
                        self.filter.rejects(FilterFacts {
                            ply: board.ply(),
                            pieces: board.pieces.occupied().count(),
                            eval_cp: eval,
                            tactical: board.is_tactical(mv),
                            in_check: board.in_check(),
                        })
                    },
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
        ACTIVATION_QUANT, EpochPlan, FilterFacts, OUTPUT_BIAS_QUANT, OUTPUT_QUANT, PlanError,
        PositionFilter, deterministic_shuffle, merge_factorised_raw_tensors,
        merge_factorised_raw_tensors_with_output_heads,
        merge_factorised_raw_tensors_with_output_shape,
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

    #[test]
    fn bullet_default_filter_matches_the_pinned_viriformat_contract() {
        let accepted = FilterFacts {
            ply: 16,
            pieces: 4,
            eval_cp: 31_338,
            tactical: false,
            in_check: false,
        };
        assert!(!PositionFilter::BulletDefault.rejects(accepted));
        assert!(!PositionFilter::None.rejects(accepted));

        for rejected in [
            FilterFacts {
                ply: 15,
                ..accepted
            },
            FilterFacts {
                pieces: 3,
                ..accepted
            },
            FilterFacts {
                eval_cp: 31_339,
                ..accepted
            },
            FilterFacts {
                eval_cp: -31_339,
                ..accepted
            },
            FilterFacts {
                tactical: true,
                ..accepted
            },
            FilterFacts {
                in_check: true,
                ..accepted
            },
        ] {
            assert!(PositionFilter::BulletDefault.rejects(rejected));
            assert!(!PositionFilter::None.rejects(rejected));
        }
    }

    #[test]
    fn factorised_raw_export_merges_shared_weights_in_bullet_save_order() {
        let tensors = vec![
            ("l0w".to_string(), vec![1.0, 2.0, 3.0, 4.0]),
            ("l0f".to_string(), vec![10.0, 20.0]),
            ("l0b".to_string(), vec![30.0]),
            ("l1w".to_string(), vec![40.0, 50.0]),
            ("l1b".to_string(), vec![60.0]),
        ];

        assert_eq!(
            merge_factorised_raw_tensors(&tensors, 2, 1, 2).unwrap(),
            vec![11.0, 22.0, 13.0, 24.0, 30.0, 40.0, 50.0, 60.0]
        );
        assert!(merge_factorised_raw_tensors(&tensors[..4], 2, 1, 2).is_err());
    }

    #[test]
    fn factorised_raw_export_transposes_multiple_output_heads() {
        let tensors = vec![
            ("l0w".to_string(), vec![1.0, 2.0]),
            ("l0f".to_string(), vec![10.0, 20.0]),
            ("l0b".to_string(), vec![30.0]),
            ("l1w".to_string(), vec![40.0, 41.0, 50.0, 51.0]),
            ("l1b".to_string(), vec![60.0, 61.0]),
        ];

        assert_eq!(
            merge_factorised_raw_tensors_with_output_heads(&tensors, 1, 1, 2, 2).unwrap(),
            vec![11.0, 22.0, 30.0, 40.0, 50.0, 41.0, 51.0, 60.0, 61.0]
        );
    }

    #[test]
    fn factorised_raw_export_accepts_a_wider_output_input() {
        let tensors = vec![
            ("l0w".to_string(), vec![1.0, 2.0]),
            ("l0f".to_string(), vec![10.0, 20.0]),
            ("l0b".to_string(), vec![30.0]),
            ("l1w".to_string(), vec![40.0, 50.0, 60.0]),
            ("l1b".to_string(), vec![70.0]),
        ];

        assert_eq!(
            merge_factorised_raw_tensors_with_output_shape(&tensors, 1, 1, 2, 3, 1).unwrap(),
            vec![11.0, 22.0, 30.0, 40.0, 50.0, 60.0, 70.0]
        );
    }
}
