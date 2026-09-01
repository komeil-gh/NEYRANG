use std::fmt;

use neyrang::chess::Color;

use crate::{AccumulatorPair, INPUT_FEATURES};

/// Hidden width of the deliberately small N0 reference topology.
pub const HIDDEN_SIZE: usize = 128;
/// Version of the NEYRANG NNUE artifact contract.
pub const FORMAT_VERSION: u16 = 1;
/// Identifier for the dual-perspective Chess768 feature mapping.
pub const FEATURE_SET_CHESS768: u16 = 1;
/// Byte length of the fixed network header.
pub const HEADER_SIZE: usize = 32;

const MAGIC: &[u8; 8] = b"NEYRANGNNUE";
const FEATURE_WEIGHT_COUNT: usize = INPUT_FEATURES * HIDDEN_SIZE;
const FEATURE_BIAS_COUNT: usize = HIDDEN_SIZE;
const OUTPUT_WEIGHT_COUNT: usize = 2 * HIDDEN_SIZE;
const PAYLOAD_SIZE: usize =
    (FEATURE_WEIGHT_COUNT + FEATURE_BIAS_COUNT + OUTPUT_WEIGHT_COUNT) * 2 + 4;

/// Quantization values carried by every network instead of hidden constants.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NetworkParameters {
    pub activation_quant: u16,
    pub output_quant: u16,
    pub centipawn_scale: i32,
}

/// A validated quantized network in the N0 reference topology.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Network {
    pub(crate) parameters: NetworkParameters,
    pub(crate) feature_weights: Vec<i16>,
    pub(crate) feature_bias: Vec<i16>,
    pub(crate) output_weights: Vec<i16>,
    pub(crate) output_bias: i32,
}

/// Fail-closed network construction and decoding errors.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NetworkError {
    BadMagic,
    UnsupportedVersion(u16),
    UnsupportedFeatureSet(u16),
    UnsupportedHiddenSize(u16),
    InvalidQuantization,
    InvalidCentipawnScale(i32),
    ReservedField(u16),
    ShapeMismatch {
        field: &'static str,
        expected: usize,
        actual: usize,
    },
    LengthMismatch {
        expected: usize,
        actual: usize,
    },
    ChecksumMismatch {
        expected: u32,
        actual: u32,
    },
    InvalidBulletPadding,
}

impl fmt::Display for NetworkError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BadMagic => formatter.write_str("invalid NEYRANG NNUE magic"),
            Self::UnsupportedVersion(version) => {
                write!(formatter, "unsupported NEYRANG NNUE format version {version}")
            }
            Self::UnsupportedFeatureSet(feature_set) => {
                write!(formatter, "unsupported NEYRANG NNUE feature set {feature_set}")
            }
            Self::UnsupportedHiddenSize(hidden_size) => {
                write!(formatter, "unsupported NEYRANG NNUE hidden size {hidden_size}")
            }
            Self::InvalidQuantization => {
                formatter.write_str("quantization scales must be non-zero")
            }
            Self::InvalidCentipawnScale(scale) => {
                write!(formatter, "centipawn scale must be positive, got {scale}")
            }
            Self::ReservedField(value) => {
                write!(
                    formatter,
                    "reserved NEYRANG NNUE header field must be zero, got {value}"
                )
            }
            Self::ShapeMismatch {
                field,
                expected,
                actual,
            } => write!(
                formatter,
                "{field} has {actual} values; expected exactly {expected}"
            ),
            Self::LengthMismatch { expected, actual } => write!(
                formatter,
                "network has {actual} bytes; expected exactly {expected}"
            ),
            Self::ChecksumMismatch { expected, actual } => write!(
                formatter,
                "network payload checksum {actual:08x} does not match {expected:08x}"
            ),
            Self::InvalidBulletPadding => {
                formatter.write_str("Bullet tensor stream has invalid 64-byte padding")
            }
        }
    }
}

impl std::error::Error for NetworkError {}

impl Network {
    /// Quantization and centipawn scaling validated by the artifact decoder.
    #[must_use]
    pub const fn parameters(&self) -> NetworkParameters {
        self.parameters
    }

    /// Construct a network from trainer/exporter tensors after exact shape validation.
    pub fn new(
        parameters: NetworkParameters,
        feature_weights: Vec<i16>,
        feature_bias: Vec<i16>,
        output_weights: Vec<i16>,
        output_bias: i32,
    ) -> Result<Self, NetworkError> {
        validate_parameters(parameters)?;
        validate_shape(
            "feature_weights",
            FEATURE_WEIGHT_COUNT,
            feature_weights.len(),
        )?;
        validate_shape("feature_bias", FEATURE_BIAS_COUNT, feature_bias.len())?;
        validate_shape("output_weights", OUTPUT_WEIGHT_COUNT, output_weights.len())?;

        Ok(Self {
            parameters,
            feature_weights,
            feature_bias,
            output_weights,
            output_bias,
        })
    }

    /// Decode and validate an NEYRANG NNUE network artifact.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, NetworkError> {
        if bytes.len() < HEADER_SIZE {
            return Err(NetworkError::LengthMismatch {
                expected: HEADER_SIZE,
                actual: bytes.len(),
            });
        }
        if &bytes[..MAGIC.len()] != MAGIC {
            return Err(NetworkError::BadMagic);
        }

        let version = read_u16(bytes, 8);
        if version != FORMAT_VERSION {
            return Err(NetworkError::UnsupportedVersion(version));
        }
        let feature_set = read_u16(bytes, 10);
        if feature_set != FEATURE_SET_CHESS768 {
            return Err(NetworkError::UnsupportedFeatureSet(feature_set));
        }
        let hidden_size = read_u16(bytes, 12);
        if usize::from(hidden_size) != HIDDEN_SIZE {
            return Err(NetworkError::UnsupportedHiddenSize(hidden_size));
        }

        let parameters = NetworkParameters {
            activation_quant: read_u16(bytes, 14),
            output_quant: read_u16(bytes, 16),
            centipawn_scale: read_i32(bytes, 20),
        };
        validate_parameters(parameters)?;

        let reserved = read_u16(bytes, 18);
        if reserved != 0 {
            return Err(NetworkError::ReservedField(reserved));
        }

        let payload_length = read_u32(bytes, 24) as usize;
        if payload_length != PAYLOAD_SIZE {
            return Err(NetworkError::LengthMismatch {
                expected: PAYLOAD_SIZE,
                actual: payload_length,
            });
        }
        let expected_length = HEADER_SIZE + payload_length;
        if bytes.len() != expected_length {
            return Err(NetworkError::LengthMismatch {
                expected: expected_length,
                actual: bytes.len(),
            });
        }

        let payload = &bytes[HEADER_SIZE..];
        let expected_checksum = read_u32(bytes, 28);
        let actual_checksum = crc32(payload);
        if actual_checksum != expected_checksum {
            return Err(NetworkError::ChecksumMismatch {
                expected: expected_checksum,
                actual: actual_checksum,
            });
        }

        let mut cursor = 0;
        let feature_weights = read_i16_values(payload, &mut cursor, FEATURE_WEIGHT_COUNT);
        let feature_bias = read_i16_values(payload, &mut cursor, FEATURE_BIAS_COUNT);
        let output_weights = read_i16_values(payload, &mut cursor, OUTPUT_WEIGHT_COUNT);
        let output_bias = read_i32(payload, cursor);

        Self::new(
            parameters,
            feature_weights,
            feature_bias,
            output_weights,
            output_bias,
        )
    }

    /// Import the exact padded tensor stream emitted by the pinned Bullet trainer.
    ///
    /// Tensor order is feature weights, feature bias, output weights and an `i32`
    /// output bias. Bullet pads the combined stream to 64 bytes with a repeating
    /// `bullet` marker; both the length and every padding byte are checked.
    pub fn from_bullet_quantised(
        bytes: &[u8],
        parameters: NetworkParameters,
    ) -> Result<Self, NetworkError> {
        validate_parameters(parameters)?;
        let expected_length = PAYLOAD_SIZE.div_ceil(64) * 64;
        if bytes.len() != expected_length {
            return Err(NetworkError::LengthMismatch {
                expected: expected_length,
                actual: bytes.len(),
            });
        }
        let padding = &bytes[PAYLOAD_SIZE..];
        if padding
            .iter()
            .enumerate()
            .any(|(index, &byte)| byte != b"bullet"[index % 6])
        {
            return Err(NetworkError::InvalidBulletPadding);
        }

        let payload = &bytes[..PAYLOAD_SIZE];
        let mut cursor = 0;
        let feature_weights = read_i16_values(payload, &mut cursor, FEATURE_WEIGHT_COUNT);
        let feature_bias = read_i16_values(payload, &mut cursor, FEATURE_BIAS_COUNT);
        let output_weights = read_i16_values(payload, &mut cursor, OUTPUT_WEIGHT_COUNT);
        let output_bias = read_i32(payload, cursor);
        Self::new(
            parameters,
            feature_weights,
            feature_bias,
            output_weights,
            output_bias,
        )
    }

    /// Serialize in the documented little-endian NEYRANG NNUE artifact format.
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut payload = Vec::with_capacity(PAYLOAD_SIZE);
        append_i16_values(&mut payload, &self.feature_weights);
        append_i16_values(&mut payload, &self.feature_bias);
        append_i16_values(&mut payload, &self.output_weights);
        payload.extend_from_slice(&self.output_bias.to_le_bytes());
        debug_assert_eq!(payload.len(), PAYLOAD_SIZE);

        let mut bytes = Vec::with_capacity(HEADER_SIZE + PAYLOAD_SIZE);
        bytes.extend_from_slice(MAGIC);
        bytes.extend_from_slice(&FORMAT_VERSION.to_le_bytes());
        bytes.extend_from_slice(&FEATURE_SET_CHESS768.to_le_bytes());
        bytes.extend_from_slice(&(HIDDEN_SIZE as u16).to_le_bytes());
        bytes.extend_from_slice(&self.parameters.activation_quant.to_le_bytes());
        bytes.extend_from_slice(&self.parameters.output_quant.to_le_bytes());
        bytes.extend_from_slice(&0_u16.to_le_bytes());
        bytes.extend_from_slice(&self.parameters.centipawn_scale.to_le_bytes());
        bytes.extend_from_slice(&(PAYLOAD_SIZE as u32).to_le_bytes());
        bytes.extend_from_slice(&crc32(&payload).to_le_bytes());
        debug_assert_eq!(bytes.len(), HEADER_SIZE);
        bytes.extend_from_slice(&payload);
        bytes
    }

    /// Evaluate from the side-to-move perspective using scalar integer arithmetic.
    #[must_use]
    pub fn evaluate(&self, accumulators: &AccumulatorPair, side_to_move: Color) -> i32 {
        let (us, them) = accumulators.oriented(side_to_move);
        let activation_quant = i128::from(self.parameters.activation_quant);
        let mut output = 0_i128;

        for (index, &value) in us.iter().enumerate() {
            output +=
                square_clipped(value, activation_quant) * i128::from(self.output_weights[index]);
        }
        for (index, &value) in them.iter().enumerate() {
            output += square_clipped(value, activation_quant)
                * i128::from(self.output_weights[HIDDEN_SIZE + index]);
        }

        output /= activation_quant;
        output += i128::from(self.output_bias);
        output *= i128::from(self.parameters.centipawn_scale);
        output /= activation_quant * i128::from(self.parameters.output_quant);
        output.clamp(i128::from(i32::MIN), i128::from(i32::MAX)) as i32
    }
}

fn square_clipped(value: i32, activation_quant: i128) -> i128 {
    let clipped = i128::from(value).clamp(0, activation_quant);
    clipped * clipped
}

fn validate_parameters(parameters: NetworkParameters) -> Result<(), NetworkError> {
    if parameters.activation_quant == 0 || parameters.output_quant == 0 {
        return Err(NetworkError::InvalidQuantization);
    }
    if parameters.centipawn_scale <= 0 {
        return Err(NetworkError::InvalidCentipawnScale(
            parameters.centipawn_scale,
        ));
    }
    Ok(())
}

fn validate_shape(field: &'static str, expected: usize, actual: usize) -> Result<(), NetworkError> {
    if actual != expected {
        return Err(NetworkError::ShapeMismatch {
            field,
            expected,
            actual,
        });
    }
    Ok(())
}

fn append_i16_values(bytes: &mut Vec<u8>, values: &[i16]) {
    for value in values {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
}

fn read_i16_values(payload: &[u8], cursor: &mut usize, count: usize) -> Vec<i16> {
    let mut values = Vec::with_capacity(count);
    for _ in 0..count {
        values.push(i16::from_le_bytes([payload[*cursor], payload[*cursor + 1]]));
        *cursor += 2;
    }
    values
}

fn read_u16(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([bytes[offset], bytes[offset + 1]])
}

fn read_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
    ])
}

fn read_i32(bytes: &[u8], offset: usize) -> i32 {
    i32::from_le_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
    ])
}

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = u32::MAX;
    for &byte in bytes {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            let mask = 0_u32.wrapping_sub(crc & 1);
            crc = (crc >> 1) ^ (0xedb8_8320 & mask);
        }
    }
    !crc
}
