use std::fmt;

use neyrang::chess::Color;

use crate::{AccumulatorPair, FeatureSet};

/// Hidden width of the deliberately small N0 reference topology.
pub const HIDDEN_SIZE: usize = 128;
/// Version of the NEYRANG NNUE artifact contract.
pub const FORMAT_VERSION: u16 = 1;
/// Version carrying the three-bank king-relative feature contract.
pub const FORMAT_VERSION_KING_BUCKETS: u16 = 2;
/// Identifier for the dual-perspective Chess768 feature mapping.
pub const FEATURE_SET_CHESS768: u16 = 1;
/// Identifier for the three-bank horizontally mirrored Chess768 mapping.
pub const FEATURE_SET_CHESS768_KING_BUCKETS_MIRRORED_3: u16 = 2;
/// Byte length of the fixed network header.
pub const HEADER_SIZE: usize = 32;

/// Eight-byte artifact marker: the engine name followed by a NUL terminator.
const MAGIC: &[u8; 8] = b"NEYRANG\0";
const FEATURE_BIAS_COUNT: usize = HIDDEN_SIZE;
const OUTPUT_WEIGHT_COUNT: usize = 2 * HIDDEN_SIZE;

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
    pub(crate) feature_set: FeatureSet,
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
                write!(
                    formatter,
                    "unsupported NEYRANG NNUE format version {version}"
                )
            }
            Self::UnsupportedFeatureSet(feature_set) => {
                write!(
                    formatter,
                    "unsupported NEYRANG NNUE feature set {feature_set}"
                )
            }
            Self::UnsupportedHiddenSize(hidden_size) => {
                write!(
                    formatter,
                    "unsupported NEYRANG NNUE hidden size {hidden_size}"
                )
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
        Self::new_with_feature_set(
            FeatureSet::Chess768,
            parameters,
            feature_weights,
            feature_bias,
            output_weights,
            output_bias,
        )
    }

    /// Construct a network with an explicit sparse feature contract.
    pub fn new_with_feature_set(
        feature_set: FeatureSet,
        parameters: NetworkParameters,
        feature_weights: Vec<i16>,
        feature_bias: Vec<i16>,
        output_weights: Vec<i16>,
        output_bias: i32,
    ) -> Result<Self, NetworkError> {
        validate_parameters(parameters)?;
        validate_shape(
            "feature_weights",
            feature_weight_count(feature_set),
            feature_weights.len(),
        )?;
        validate_shape("feature_bias", FEATURE_BIAS_COUNT, feature_bias.len())?;
        validate_shape("output_weights", OUTPUT_WEIGHT_COUNT, output_weights.len())?;

        Ok(Self {
            feature_set,
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
        let feature_set_id = read_u16(bytes, 10);
        let feature_set = decode_feature_set(version, feature_set_id)?;
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
        let payload_size = payload_size(feature_set);
        if payload_length != payload_size {
            return Err(NetworkError::LengthMismatch {
                expected: payload_size,
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
        let feature_weights =
            read_i16_values(payload, &mut cursor, feature_weight_count(feature_set));
        let feature_bias = read_i16_values(payload, &mut cursor, FEATURE_BIAS_COUNT);
        let output_weights = read_i16_values(payload, &mut cursor, OUTPUT_WEIGHT_COUNT);
        let output_bias = read_i32(payload, cursor);

        Self::new_with_feature_set(
            feature_set,
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
        Self::from_bullet_quantised_with_feature_set(bytes, FeatureSet::Chess768, parameters)
    }

    /// Import a pinned Bullet tensor stream with an explicit sparse feature set.
    pub fn from_bullet_quantised_with_feature_set(
        bytes: &[u8],
        feature_set: FeatureSet,
        parameters: NetworkParameters,
    ) -> Result<Self, NetworkError> {
        validate_parameters(parameters)?;
        let payload_size = payload_size(feature_set);
        let expected_length = payload_size.div_ceil(64) * 64;
        if bytes.len() != expected_length {
            return Err(NetworkError::LengthMismatch {
                expected: expected_length,
                actual: bytes.len(),
            });
        }
        let padding = &bytes[payload_size..];
        if padding
            .iter()
            .enumerate()
            .any(|(index, &byte)| byte != b"bullet"[index % 6])
        {
            return Err(NetworkError::InvalidBulletPadding);
        }

        let payload = &bytes[..payload_size];
        let mut cursor = 0;
        let feature_weights =
            read_i16_values(payload, &mut cursor, feature_weight_count(feature_set));
        let feature_bias = read_i16_values(payload, &mut cursor, FEATURE_BIAS_COUNT);
        let output_weights = read_i16_values(payload, &mut cursor, OUTPUT_WEIGHT_COUNT);
        let output_bias = read_i32(payload, cursor);
        Self::new_with_feature_set(
            feature_set,
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
        let payload_size = payload_size(self.feature_set);
        let mut payload = Vec::with_capacity(payload_size);
        append_i16_values(&mut payload, &self.feature_weights);
        append_i16_values(&mut payload, &self.feature_bias);
        append_i16_values(&mut payload, &self.output_weights);
        payload.extend_from_slice(&self.output_bias.to_le_bytes());
        debug_assert_eq!(payload.len(), payload_size);

        let mut bytes = Vec::with_capacity(HEADER_SIZE + payload_size);
        bytes.extend_from_slice(MAGIC);
        bytes.extend_from_slice(&format_version(self.feature_set).to_le_bytes());
        bytes.extend_from_slice(&feature_set_id(self.feature_set).to_le_bytes());
        bytes.extend_from_slice(&(HIDDEN_SIZE as u16).to_le_bytes());
        bytes.extend_from_slice(&self.parameters.activation_quant.to_le_bytes());
        bytes.extend_from_slice(&self.parameters.output_quant.to_le_bytes());
        bytes.extend_from_slice(&0_u16.to_le_bytes());
        bytes.extend_from_slice(&self.parameters.centipawn_scale.to_le_bytes());
        bytes.extend_from_slice(&(payload_size as u32).to_le_bytes());
        bytes.extend_from_slice(&crc32(&payload).to_le_bytes());
        debug_assert_eq!(bytes.len(), HEADER_SIZE);
        bytes.extend_from_slice(&payload);
        bytes
    }

    /// Sparse feature contract carried by this validated network.
    #[must_use]
    pub const fn feature_set(&self) -> FeatureSet {
        self.feature_set
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

const fn feature_weight_count(feature_set: FeatureSet) -> usize {
    feature_set.input_features() * HIDDEN_SIZE
}

const fn payload_size(feature_set: FeatureSet) -> usize {
    (feature_weight_count(feature_set) + FEATURE_BIAS_COUNT + OUTPUT_WEIGHT_COUNT) * 2 + 4
}

const fn format_version(feature_set: FeatureSet) -> u16 {
    match feature_set {
        FeatureSet::Chess768 => FORMAT_VERSION,
        FeatureSet::Chess768KingBucketsMirrored3 => FORMAT_VERSION_KING_BUCKETS,
    }
}

const fn feature_set_id(feature_set: FeatureSet) -> u16 {
    match feature_set {
        FeatureSet::Chess768 => FEATURE_SET_CHESS768,
        FeatureSet::Chess768KingBucketsMirrored3 => FEATURE_SET_CHESS768_KING_BUCKETS_MIRRORED_3,
    }
}

fn decode_feature_set(version: u16, feature_set: u16) -> Result<FeatureSet, NetworkError> {
    match (version, feature_set) {
        (FORMAT_VERSION, FEATURE_SET_CHESS768) => Ok(FeatureSet::Chess768),
        (FORMAT_VERSION_KING_BUCKETS, FEATURE_SET_CHESS768_KING_BUCKETS_MIRRORED_3) => {
            Ok(FeatureSet::Chess768KingBucketsMirrored3)
        }
        (FORMAT_VERSION | FORMAT_VERSION_KING_BUCKETS, feature_set) => {
            Err(NetworkError::UnsupportedFeatureSet(feature_set))
        }
        (version, _) => Err(NetworkError::UnsupportedVersion(version)),
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
