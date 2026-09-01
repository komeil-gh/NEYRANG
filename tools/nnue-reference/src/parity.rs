use std::fmt;

use neyrang::chess::Position;

use crate::{AccumulatorPair, FloatNetwork, Network};

/// One raw-float versus quantized evaluation comparison.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ParitySample {
    pub float_cp: f64,
    pub quantized_cp: i32,
    pub abs_error_cp: f64,
}

/// Aggregate parity evidence for a frozen FEN suite.
#[derive(Clone, Debug, PartialEq)]
pub struct ParityReport {
    pub samples: Vec<ParitySample>,
    pub max_abs_error_cp: f64,
    pub mean_abs_error_cp: f64,
    pub minimum_accumulator: i32,
    pub maximum_accumulator: i32,
}

impl ParityReport {
    #[must_use]
    pub fn passes(&self, maximum_error_cp: f64, mean_error_cp: f64) -> bool {
        maximum_error_cp.is_finite()
            && mean_error_cp.is_finite()
            && maximum_error_cp >= 0.0
            && mean_error_cp >= 0.0
            && self.max_abs_error_cp <= maximum_error_cp
            && self.mean_abs_error_cp <= mean_error_cp
    }
}

/// Parity cannot be evaluated without at least one frozen position.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ParityError {
    EmptySuite,
}

impl fmt::Display for ParityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("parity suite must contain at least one position")
    }
}

impl std::error::Error for ParityError {}

pub fn evaluate_parity(
    float_network: &FloatNetwork,
    quantized_network: &Network,
    positions: &[Position],
) -> Result<ParityReport, ParityError> {
    if positions.is_empty() {
        return Err(ParityError::EmptySuite);
    }

    let mut samples = Vec::with_capacity(positions.len());
    let mut maximum = 0.0_f64;
    let mut sum = 0.0_f64;
    let mut minimum_accumulator = i32::MAX;
    let mut maximum_accumulator = i32::MIN;
    for position in positions {
        let float_cp = f64::from(float_network.evaluate(position));
        let accumulators = AccumulatorPair::refresh(position, quantized_network);
        let (position_minimum, position_maximum) = accumulators.range();
        minimum_accumulator = minimum_accumulator.min(position_minimum);
        maximum_accumulator = maximum_accumulator.max(position_maximum);
        let quantized_cp = quantized_network.evaluate(&accumulators, position.side_to_move());
        let error = (float_cp - f64::from(quantized_cp)).abs();
        maximum = maximum.max(error);
        sum += error;
        samples.push(ParitySample {
            float_cp,
            quantized_cp,
            abs_error_cp: error,
        });
    }

    Ok(ParityReport {
        mean_abs_error_cp: sum / samples.len() as f64,
        max_abs_error_cp: maximum,
        samples,
        minimum_accumulator,
        maximum_accumulator,
    })
}
