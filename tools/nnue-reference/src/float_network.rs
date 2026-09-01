use std::fmt;

use neyrang::chess::{Color, Position};

use crate::{HIDDEN_SIZE, INPUT_FEATURES, Network, NetworkParameters, active_features};

const FEATURE_WEIGHT_COUNT: usize = INPUT_FEATURES * HIDDEN_SIZE;
const FEATURE_BIAS_COUNT: usize = HIDDEN_SIZE;
const OUTPUT_WEIGHT_COUNT: usize = 2 * HIDDEN_SIZE;
const FLOAT_COUNT: usize = FEATURE_WEIGHT_COUNT + FEATURE_BIAS_COUNT + OUTPUT_WEIGHT_COUNT + 1;
const RAW_BYTE_COUNT: usize = FLOAT_COUNT * size_of::<f32>();

/// The unquantized `(Chess768 -> 128) x 2 -> 1` tensors saved by Bullet.
#[derive(Clone, Debug, PartialEq)]
pub struct FloatNetwork {
    feature_weights: Vec<f32>,
    feature_bias: Vec<f32>,
    output_weights: Vec<f32>,
    output_bias: f32,
    centipawn_scale: f32,
}

/// Fail-closed errors for the pinned Bullet raw tensor stream.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FloatNetworkError {
    LengthMismatch { expected: usize, actual: usize },
    NonFiniteWeight { index: usize },
    InvalidCentipawnScale,
}

/// Quantization errors mirror Bullet's refusal to truncate out-of-range tensors.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FloatQuantizationError {
    InvalidActivationQuant(u16),
    InvalidOutputQuant(u16),
    CentipawnScaleMismatch,
    BiasScaleOverflow,
    I16OutOfRange { tensor: &'static str, index: usize },
    I32OutOfRange { tensor: &'static str, index: usize },
}

impl fmt::Display for FloatNetworkError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LengthMismatch { expected, actual } => write!(
                formatter,
                "raw Bullet tensor stream has {actual} bytes; expected exactly {expected}"
            ),
            Self::NonFiniteWeight { index } => {
                write!(formatter, "raw Bullet tensor {index} is not finite")
            }
            Self::InvalidCentipawnScale => {
                formatter.write_str("centipawn scale must be finite and positive")
            }
        }
    }
}

impl std::error::Error for FloatNetworkError {}

impl fmt::Display for FloatQuantizationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidActivationQuant(value) => {
                write!(
                    formatter,
                    "activation quantization {value} does not fit Bullet i16"
                )
            }
            Self::InvalidOutputQuant(value) => {
                write!(
                    formatter,
                    "output quantization {value} does not fit Bullet i16"
                )
            }
            Self::CentipawnScaleMismatch => {
                formatter.write_str("integer artifact centipawn scale differs from raw evaluator")
            }
            Self::BiasScaleOverflow => {
                formatter.write_str("activation/output quantization product overflows i32")
            }
            Self::I16OutOfRange { tensor, index } => {
                write!(formatter, "{tensor} value {index} does not quantize to i16")
            }
            Self::I32OutOfRange { tensor, index } => {
                write!(formatter, "{tensor} value {index} does not quantize to i32")
            }
        }
    }
}

impl std::error::Error for FloatQuantizationError {}

impl FloatNetwork {
    /// Decode the exact unpadded f32 tensor order emitted by the pinned trainer.
    pub fn from_bullet_raw(bytes: &[u8], centipawn_scale: f32) -> Result<Self, FloatNetworkError> {
        if bytes.len() != RAW_BYTE_COUNT {
            return Err(FloatNetworkError::LengthMismatch {
                expected: RAW_BYTE_COUNT,
                actual: bytes.len(),
            });
        }
        if !centipawn_scale.is_finite() || centipawn_scale <= 0.0 {
            return Err(FloatNetworkError::InvalidCentipawnScale);
        }

        let (chunks, remainder) = bytes.as_chunks::<4>();
        debug_assert!(remainder.is_empty());
        let mut values = Vec::with_capacity(FLOAT_COUNT);
        for (index, bytes) in chunks.iter().enumerate() {
            let value = f32::from_le_bytes(*bytes);
            if !value.is_finite() {
                return Err(FloatNetworkError::NonFiniteWeight { index });
            }
            values.push(value);
        }

        let feature_bias_start = FEATURE_WEIGHT_COUNT;
        let output_weight_start = feature_bias_start + FEATURE_BIAS_COUNT;
        let output_bias_index = output_weight_start + OUTPUT_WEIGHT_COUNT;
        Ok(Self {
            feature_weights: values[..feature_bias_start].to_vec(),
            feature_bias: values[feature_bias_start..output_weight_start].to_vec(),
            output_weights: values[output_weight_start..output_bias_index].to_vec(),
            output_bias: values[output_bias_index],
            centipawn_scale,
        })
    }

    /// Evaluate the raw float graph from the side-to-move perspective.
    #[must_use]
    pub fn evaluate(&self, position: &Position) -> f32 {
        let side_to_move = position.side_to_move();
        let other = match side_to_move {
            Color::White => Color::Black,
            Color::Black => Color::White,
        };
        let us = self.hidden(position, side_to_move);
        let them = self.hidden(position, other);

        let mut output = self.output_bias;
        for (index, value) in us.into_iter().enumerate() {
            output += screlu(value) * self.output_weights[index];
        }
        for (index, value) in them.into_iter().enumerate() {
            output += screlu(value) * self.output_weights[HIDDEN_SIZE + index];
        }
        output * self.centipawn_scale
    }

    /// Apply Bullet's round-to-nearest tensor quantization without saturation.
    pub fn quantize(
        &self,
        parameters: NetworkParameters,
    ) -> Result<Network, FloatQuantizationError> {
        let activation_quant = i16::try_from(parameters.activation_quant)
            .ok()
            .filter(|value| *value > 0)
            .ok_or(FloatQuantizationError::InvalidActivationQuant(
                parameters.activation_quant,
            ))?;
        let output_quant = i16::try_from(parameters.output_quant)
            .ok()
            .filter(|value| *value > 0)
            .ok_or(FloatQuantizationError::InvalidOutputQuant(
                parameters.output_quant,
            ))?;
        if parameters.centipawn_scale as f32 != self.centipawn_scale {
            return Err(FloatQuantizationError::CentipawnScaleMismatch);
        }
        let bias_quant = i32::from(activation_quant)
            .checked_mul(i32::from(output_quant))
            .ok_or(FloatQuantizationError::BiasScaleOverflow)?;

        let feature_weights =
            quantize_i16("feature_weights", &self.feature_weights, activation_quant)?;
        let feature_bias = quantize_i16("feature_bias", &self.feature_bias, activation_quant)?;
        let output_weights = quantize_i16("output_weights", &self.output_weights, output_quant)?;
        let output_bias = quantize_i32("output_bias", &[self.output_bias], bias_quant)?[0];

        Ok(Network::new(
            parameters,
            feature_weights,
            feature_bias,
            output_weights,
            output_bias,
        )
        .expect("validated quantization and fixed tensor shapes form a valid network"))
    }

    fn hidden(&self, position: &Position, perspective: Color) -> [f32; HIDDEN_SIZE] {
        let mut hidden = [0.0; HIDDEN_SIZE];
        hidden.copy_from_slice(&self.feature_bias);
        for feature in active_features(position, perspective) {
            let start = feature * HIDDEN_SIZE;
            for (value, &weight) in hidden
                .iter_mut()
                .zip(&self.feature_weights[start..start + HIDDEN_SIZE])
            {
                *value += weight;
            }
        }
        hidden
    }
}

fn screlu(value: f32) -> f32 {
    value.clamp(0.0, 1.0).powi(2)
}

fn quantize_i16(
    tensor: &'static str,
    values: &[f32],
    multiplier: i16,
) -> Result<Vec<i16>, FloatQuantizationError> {
    values
        .iter()
        .enumerate()
        .map(|(index, &value)| {
            let quantized = (f64::from(value) * f64::from(multiplier)).round();
            if quantized < f64::from(i16::MIN) || quantized > f64::from(i16::MAX) {
                return Err(FloatQuantizationError::I16OutOfRange { tensor, index });
            }
            Ok(quantized as i16)
        })
        .collect()
}

fn quantize_i32(
    tensor: &'static str,
    values: &[f32],
    multiplier: i32,
) -> Result<Vec<i32>, FloatQuantizationError> {
    values
        .iter()
        .enumerate()
        .map(|(index, &value)| {
            let quantized = (f64::from(value) * f64::from(multiplier)).round();
            if quantized < f64::from(i32::MIN) || quantized > f64::from(i32::MAX) {
                return Err(FloatQuantizationError::I32OutOfRange { tensor, index });
            }
            Ok(quantized as i32)
        })
        .collect()
}
