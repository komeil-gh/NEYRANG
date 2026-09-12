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
    CorpusCompositionReport, NamedSignSliceReport, NetworkDiagnosticError, NetworkDiagnosticReport,
    PairedComparisonError, PairedComparisonReport, PairedSignDiagnosticReport,
    SEARCH_LABEL_MATE_BOUND, ScoreFitReport, SignTransitionReport, blended_target,
    compare_networks, diagnose_network, diagnose_sign_disagreements, logistic_cp,
};
pub use features::{
    FeatureSet, INPUT_FEATURES, INPUT_FEATURES_KING_BUCKETS_MIRRORED_3,
    KING_BUCKET_LAYOUT_MIRRORED_3, active_features, active_features_for, feature_index,
    feature_index_for, king_bucket_mirrored_3,
};
pub use fen_suite::{FenSuiteError, FrozenPosition, parse_fen_suite};
pub use float_network::{FloatNetwork, FloatNetworkError, FloatQuantizationError};
pub use king_buckets::{KingBucketError, KingBucketReport, analyze_king_buckets};
pub use network::{
    FEATURE_SET_CHESS768, FEATURE_SET_CHESS768_KING_BUCKETS_MIRRORED_3, FORMAT_VERSION,
    FORMAT_VERSION_KING_BUCKETS, HEADER_SIZE, HIDDEN_SIZE, Network, NetworkError,
    NetworkParameters,
};
pub use parity::{ParityError, ParityReport, ParitySample, evaluate_parity};
