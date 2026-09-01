//! Scalar correctness oracle for NEYRANG NNUE experiments.

mod accumulator;
mod diagnostics;
mod features;
mod fen_suite;
mod float_network;
mod king_buckets;
mod network;
mod parity;

pub use accumulator::AccumulatorPair;
pub use diagnostics::{
    CorpusCompositionReport, NetworkDiagnosticError, NetworkDiagnosticReport,
    PairedComparisonError, PairedComparisonReport, ScoreFitReport, blended_target,
    compare_networks, diagnose_network, logistic_cp,
};
pub use features::{INPUT_FEATURES, active_features, feature_index};
pub use fen_suite::{FenSuiteError, FrozenPosition, parse_fen_suite};
pub use float_network::{FloatNetwork, FloatNetworkError, FloatQuantizationError};
pub use king_buckets::{KingBucketError, KingBucketReport, analyze_king_buckets};
pub use network::{
    FEATURE_SET_CHESS768, FORMAT_VERSION, HEADER_SIZE, HIDDEN_SIZE, Network, NetworkError,
    NetworkParameters,
};
pub use parity::{ParityError, ParityReport, ParitySample, evaluate_parity};
