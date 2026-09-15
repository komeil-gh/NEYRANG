use std::fmt;

use neyrang::chess::{Color, Position};

use crate::{FeatureSet, HIDDEN_SIZE, Network, NetworkParameters, active_features_for};

const FEATURE_BIAS_COUNT: usize = HIDDEN_SIZE;

/// The unquantized `(Chess768 -> 128) x 2 -> 1` tensors saved by Bullet.
#[derive(Clone, Debug, PartialEq)]
pub struct FloatNetwork {
    feature_set: FeatureSet,
    feature_weights: Vec<f32>,
    feature_bias: Vec<f32>,
    output_weights: Vec<f32>,
    output_bias: Vec<f32>,
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
    /// Sparse feature contract carried by the raw trainer tensors.
    #[must_use]
    pub const fn feature_set(&self) -> FeatureSet {
        self.feature_set
    }

    /// Decode the exact unpadded f32 tensor order emitted by the pinned trainer.
    pub fn from_bullet_raw(bytes: &[u8], centipawn_scale: f32) -> Result<Self, FloatNetworkError> {
        Self::from_bullet_raw_with_feature_set(bytes, centipawn_scale, FeatureSet::Chess768)
    }

    /// Decode a pinned Bullet tensor stream with an explicit sparse feature set.
    pub fn from_bullet_raw_with_feature_set(
        bytes: &[u8],
        centipawn_scale: f32,
        feature_set: FeatureSet,
    ) -> Result<Self, FloatNetworkError> {
        let feature_weight_count = feature_set.input_features() * HIDDEN_SIZE;
        let output_weight_count = 2 * HIDDEN_SIZE * feature_set.output_heads();
        let output_bias_count = feature_set.output_heads();
        let float_count =
            feature_weight_count + FEATURE_BIAS_COUNT + output_weight_count + output_bias_count;
        let raw_byte_count = float_count * size_of::<f32>();
        if bytes.len() != raw_byte_count {
            return Err(FloatNetworkError::LengthMismatch {
                expected: raw_byte_count,
                actual: bytes.len(),
            });
        }
        if !centipawn_scale.is_finite() || centipawn_scale <= 0.0 {
            return Err(FloatNetworkError::InvalidCentipawnScale);
        }

        let (chunks, remainder) = bytes.as_chunks::<4>();
        debug_assert!(remainder.is_empty());
        let mut values = Vec::with_capacity(float_count);
        for (index, bytes) in chunks.iter().enumerate() {
            let value = f32::from_le_bytes(*bytes);
            if !value.is_finite() {
                return Err(FloatNetworkError::NonFiniteWeight { index });
            }
            values.push(value);
        }

        let feature_bias_start = feature_weight_count;
        let output_weight_start = feature_bias_start + FEATURE_BIAS_COUNT;
        let output_bias_start = output_weight_start + output_weight_count;
        Ok(Self {
            feature_set,
            feature_weights: values[..feature_bias_start].to_vec(),
            feature_bias: values[feature_bias_start..output_weight_start].to_vec(),
            output_weights: values[output_weight_start..output_bias_start].to_vec(),
            output_bias: values[output_bias_start..].to_vec(),
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

        let head = material_output_head(
            position.all_occupancy().count_ones() as u8,
            self.feature_set.output_heads(),
        );
        let weight_offset = head * 2 * HIDDEN_SIZE;
        let mut output = self.output_bias[head];
        for (index, value) in us.into_iter().enumerate() {
            output += screlu(value) * self.output_weights[weight_offset + index];
        }
        for (index, value) in them.into_iter().enumerate() {
            output += screlu(value) * self.output_weights[weight_offset + HIDDEN_SIZE + index];
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
        let output_bias = quantize_i32("output_bias", &self.output_bias, bias_quant)?;

        Ok(Network::new_with_feature_set_and_output_biases(
            self.feature_set,
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
        for feature in active_features_for(position, perspective, self.feature_set) {
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

fn material_output_head(piece_count: u8, head_count: usize) -> usize {
    let divisor = 32_usize.div_ceil(head_count);
    (usize::from(piece_count) - 2) / divisor
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
