//! Scalar correctness oracle for NEYRANG NNUE experiments.

mod accumulator;
mod features;
mod network;

pub use accumulator::AccumulatorPair;
pub use features::{INPUT_FEATURES, active_features, feature_index};
pub use network::{
    FEATURE_SET_CHESS768, FORMAT_VERSION, HEADER_SIZE, HIDDEN_SIZE, Network, NetworkError,
    NetworkParameters,
};
