//! Scalar NNUE inference owned by SANJ.
//!
//! This is the correctness path for the `NEYRANG\0` Chess768 and
//! Chess768x3hm artifacts. King moves deliberately refresh king-bucket
//! accumulators; other moves use the independently tested incremental path.

use std::fmt;

use crate::chess::{Color, Move, MoveFlag, PieceType, Position, Square};

/// Two colors, six piece kinds and 64 squares.
pub const INPUT_FEATURES: usize = 2 * 6 * 64;
/// Three horizontally mirrored own-king banks of Chess768 features.
pub const INPUT_FEATURES_KING_BUCKETS_MIRRORED_3: usize = 3 * INPUT_FEATURES;
/// Hidden width frozen by the version-1 artifact contract.
pub const HIDDEN_SIZE: usize = 128;
/// Version of the NEYRANG NNUE artifact contract.
pub const FORMAT_VERSION: u16 = 1;
/// Artifact version for the registered three-bank king-relative mapping.
pub const FORMAT_VERSION_KING_BUCKETS: u16 = 2;
/// Identifier for the dual-perspective Chess768 feature mapping.
pub const FEATURE_SET_CHESS768: u16 = 1;
/// Identifier for Chess768x3hm.
pub const FEATURE_SET_CHESS768_KING_BUCKETS_MIRRORED_3: u16 = 2;
/// Byte length of the fixed artifact header.
pub const HEADER_SIZE: usize = 32;

const MAGIC: &[u8; 8] = b"NEYRANG\0";
const FEATURE_BIAS_COUNT: usize = HIDDEN_SIZE;
const OUTPUT_WEIGHT_COUNT: usize = 2 * HIDDEN_SIZE;
const KING_BUCKET_LAYOUT_MIRRORED_3: [u8; 32] = [
    1, 1, 1, 0, // home rank
    1, 1, 1, 1, // second rank
    2, 2, 2, 2, // active king ranks
    2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2,
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FeatureSet {
    Chess768,
    Chess768KingBucketsMirrored3,
}

impl FeatureSet {
    const fn input_features(self) -> usize {
        match self {
            Self::Chess768 => INPUT_FEATURES,
            Self::Chess768KingBucketsMirrored3 => INPUT_FEATURES_KING_BUCKETS_MIRRORED_3,
        }
    }
}

/// Quantization values validated from the artifact header.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NetworkParameters {
    pub activation_quant: u16,
    pub output_quant: u16,
    pub centipawn_scale: i32,
}

/// A validated scalar network ready for full-refresh SANJ inference.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Network {
    feature_set: FeatureSet,
    parameters: NetworkParameters,
    feature_weights: Box<[i16]>,
    feature_bias: Box<[i16]>,
    output_weights: Box<[i16]>,
    output_bias: i32,
}

/// Both board-oriented hidden accumulators for one search position.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AccumulatorPair {
    white: [i32; HIDDEN_SIZE],
    black: [i32; HIDDEN_SIZE],
}

impl AccumulatorPair {
    /// Rebuild both perspectives from the complete board.
    #[must_use]
    pub fn refresh(position: &Position, network: &Network) -> Self {
        let mut pair = Self {
            white: [0; HIDDEN_SIZE],
            black: [0; HIDDEN_SIZE],
        };
        for ((white_value, black_value), &bias) in pair
            .white
            .iter_mut()
            .zip(&mut pair.black)
            .zip(network.feature_bias.iter())
        {
            *white_value = i32::from(bias);
            *black_value = i32::from(bias);
        }

        for index in 0_u8..64 {
            let square = Square::from_index(index).expect("board index is in range");
            if let Some((color, piece_type)) = position.piece_at(square) {
                pair.add_piece(position, color, piece_type, square, network);
            }
        }
        pair
    }

    /// Derive the child accumulator from one legal move without scanning the board.
    #[must_use]
    pub fn after_move(&self, position: &Position, mv: Move, network: &Network) -> Self {
        let mut next = self.clone();
        let from = mv.from();
        let to = mv.to();
        let (moving_color, moving_type) = position
            .piece_at(from)
            .expect("a legal move source must contain a piece");
        if network.feature_set == FeatureSet::Chess768KingBucketsMirrored3
            && moving_type == PieceType::King
        {
            let mut child = position.clone();
            child.make_move(mv);
            return Self::refresh(&child, network);
        }

        next.remove_piece(position, moving_color, moving_type, from, network);

        if mv.is_capture() {
            let capture_square = if mv.is_en_passant() {
                Square::from_coords(to.file(), from.rank())
                    .expect("en-passant capture square is on the board")
            } else {
                to
            };
            let (captured_color, captured_type) = position
                .piece_at(capture_square)
                .expect("a legal capture must identify the captured piece");
            next.remove_piece(
                position,
                captured_color,
                captured_type,
                capture_square,
                network,
            );
        }

        if mv.is_castle() {
            let (rook_from, rook_to) = castle_rook_squares(moving_color, mv.flag());
            next.remove_piece(position, moving_color, PieceType::Rook, rook_from, network);
            next.add_piece(position, moving_color, PieceType::Rook, rook_to, network);
        }

        next.add_piece(
            position,
            moving_color,
            mv.promotion().unwrap_or(moving_type),
            to,
            network,
        );
        next
    }

    fn oriented(&self, side_to_move: Color) -> (&[i32; HIDDEN_SIZE], &[i32; HIDDEN_SIZE]) {
        match side_to_move {
            Color::White => (&self.white, &self.black),
            Color::Black => (&self.black, &self.white),
        }
    }

    fn add_piece(
        &mut self,
        position: &Position,
        color: Color,
        piece_type: PieceType,
        square: Square,
        network: &Network,
    ) {
        network.add_feature(
            &mut self.white,
            feature_index_for(
                position,
                color,
                piece_type,
                square,
                Color::White,
                network.feature_set,
            ),
        );
        network.add_feature(
            &mut self.black,
            feature_index_for(
                position,
                color,
                piece_type,
                square,
                Color::Black,
                network.feature_set,
            ),
        );
    }

    fn remove_piece(
        &mut self,
        position: &Position,
        color: Color,
        piece_type: PieceType,
        square: Square,
        network: &Network,
    ) {
        network.remove_feature(
            &mut self.white,
            feature_index_for(
                position,
                color,
                piece_type,
                square,
                Color::White,
                network.feature_set,
            ),
        );
        network.remove_feature(
            &mut self.black,
            feature_index_for(
                position,
                color,
                piece_type,
                square,
                Color::Black,
                network.feature_set,
            ),
        );
    }
}

/// Fail-closed artifact decoding errors.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NetworkError {
    BadMagic,
    UnsupportedVersion(u16),
    UnsupportedFeatureSet(u16),
    UnsupportedHiddenSize(u16),
    InvalidQuantization,
    InvalidCentipawnScale(i32),
    ReservedField(u16),
    LengthMismatch { expected: usize, actual: usize },
    ChecksumMismatch { expected: u32, actual: u32 },
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
            Self::ReservedField(value) => write!(
                formatter,
                "reserved NEYRANG NNUE header field must be zero, got {value}"
            ),
            Self::LengthMismatch { expected, actual } => write!(
                formatter,
                "network has {actual} bytes; expected exactly {expected}"
            ),
            Self::ChecksumMismatch { expected, actual } => write!(
                formatter,
                "network payload checksum {actual:08x} does not match {expected:08x}"
            ),
        }
    }
}

impl std::error::Error for NetworkError {}

impl Network {
    /// Decode a complete supported NEYRANG artifact and reject any drift.
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
        let feature_set = match (version, feature_set_id) {
            (FORMAT_VERSION, FEATURE_SET_CHESS768) => FeatureSet::Chess768,
            (FORMAT_VERSION_KING_BUCKETS, FEATURE_SET_CHESS768_KING_BUCKETS_MIRRORED_3) => {
                FeatureSet::Chess768KingBucketsMirrored3
            }
            (FORMAT_VERSION | FORMAT_VERSION_KING_BUCKETS, feature_set) => {
                return Err(NetworkError::UnsupportedFeatureSet(feature_set));
            }
            (version, _) => return Err(NetworkError::UnsupportedVersion(version)),
        };
        let hidden_size = read_u16(bytes, 12);
        if usize::from(hidden_size) != HIDDEN_SIZE {
            return Err(NetworkError::UnsupportedHiddenSize(hidden_size));
        }

        let parameters = NetworkParameters {
            activation_quant: read_u16(bytes, 14),
            output_quant: read_u16(bytes, 16),
            centipawn_scale: read_i32(bytes, 20),
        };
        if parameters.activation_quant == 0 || parameters.output_quant == 0 {
            return Err(NetworkError::InvalidQuantization);
        }
        if parameters.centipawn_scale <= 0 {
            return Err(NetworkError::InvalidCentipawnScale(
                parameters.centipawn_scale,
            ));
        }

        let reserved = read_u16(bytes, 18);
        if reserved != 0 {
            return Err(NetworkError::ReservedField(reserved));
        }

        let payload_length = read_u32(bytes, 24) as usize;
        let feature_weight_count = feature_set.input_features() * HIDDEN_SIZE;
        let payload_size =
            (feature_weight_count + FEATURE_BIAS_COUNT + OUTPUT_WEIGHT_COUNT) * 2 + 4;
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
        let feature_weights = read_i16_values(payload, &mut cursor, feature_weight_count);
        let feature_bias = read_i16_values(payload, &mut cursor, FEATURE_BIAS_COUNT);
        let output_weights = read_i16_values(payload, &mut cursor, OUTPUT_WEIGHT_COUNT);
        let output_bias = read_i32(payload, cursor);

        Ok(Self {
            feature_set,
            parameters,
            feature_weights: feature_weights.into_boxed_slice(),
            feature_bias: feature_bias.into_boxed_slice(),
            output_weights: output_weights.into_boxed_slice(),
            output_bias,
        })
    }

    /// Quantization and score scale carried by the decoded artifact.
    #[must_use]
    pub const fn parameters(&self) -> NetworkParameters {
        self.parameters
    }

    /// Evaluate with a full dual-perspective accumulator refresh.
    #[must_use]
    pub fn evaluate(&self, position: &Position) -> i32 {
        let accumulators = AccumulatorPair::refresh(position, self);
        self.evaluate_accumulator(&accumulators, position.side_to_move())
    }

    /// Evaluate a previously refreshed or incrementally updated accumulator.
    #[must_use]
    pub fn evaluate_accumulator(&self, accumulators: &AccumulatorPair, side_to_move: Color) -> i32 {
        let (us, them) = accumulators.oriented(side_to_move);
        self.activate(us, them)
    }

    fn add_feature(&self, accumulator: &mut [i32; HIDDEN_SIZE], feature: usize) {
        let start = feature * HIDDEN_SIZE;
        for (target, &weight) in accumulator
            .iter_mut()
            .zip(&self.feature_weights[start..start + HIDDEN_SIZE])
        {
            *target += i32::from(weight);
        }
    }

    fn remove_feature(&self, accumulator: &mut [i32; HIDDEN_SIZE], feature: usize) {
        let start = feature * HIDDEN_SIZE;
        for (target, &weight) in accumulator
            .iter_mut()
            .zip(&self.feature_weights[start..start + HIDDEN_SIZE])
        {
            *target -= i32::from(weight);
        }
    }

    fn activate(&self, us: &[i32; HIDDEN_SIZE], them: &[i32; HIDDEN_SIZE]) -> i32 {
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

fn castle_rook_squares(color: Color, flag: MoveFlag) -> (Square, Square) {
    match (color, flag) {
        (Color::White, MoveFlag::KingCastle) => (Square::H1, Square::F1),
        (Color::White, MoveFlag::QueenCastle) => (Square::A1, Square::D1),
        (Color::Black, MoveFlag::KingCastle) => (Square::H8, Square::F8),
        (Color::Black, MoveFlag::QueenCastle) => (Square::A8, Square::D8),
        _ => unreachable!("only castling moves relocate a rook"),
    }
}

#[inline]
const fn feature_index(
    piece_color: Color,
    piece_type: PieceType,
    square: Square,
    perspective: Color,
) -> usize {
    let relative_color = if piece_color as u8 == perspective as u8 {
        0
    } else {
        1
    };
    let oriented_square = match perspective {
        Color::White => square.index(),
        Color::Black => square.index() ^ 56,
    };
    (relative_color * 6 + piece_type.index()) * 64 + oriented_square
}

#[inline]
fn feature_index_for(
    position: &Position,
    piece_color: Color,
    piece_type: PieceType,
    square: Square,
    perspective: Color,
    feature_set: FeatureSet,
) -> usize {
    let base = feature_index(piece_color, piece_type, square, perspective);
    match feature_set {
        FeatureSet::Chess768 => base,
        FeatureSet::Chess768KingBucketsMirrored3 => {
            let king_square = position
                .pieces(perspective, PieceType::King)
                .trailing_zeros() as usize;
            let oriented_king = match perspective {
                Color::White => king_square,
                Color::Black => king_square ^ 56,
            };
            let rank = oriented_king / 8;
            let file = oriented_king % 8;
            let mirrored_file = file.min(7 - file);
            let bucket = usize::from(KING_BUCKET_LAYOUT_MIRRORED_3[rank * 4 + mirrored_file]);
            let horizontal_flip = if file > 3 { 7 } else { 0 };
            bucket * INPUT_FEATURES + (base ^ horizontal_flip)
        }
    }
}

#[inline]
fn square_clipped(value: i32, activation_quant: i128) -> i128 {
    let clipped = i128::from(value).clamp(0, activation_quant);
    clipped * clipped
}

fn read_i16_values(payload: &[u8], cursor: &mut usize, count: usize) -> Vec<i16> {
    let mut values = Vec::with_capacity(count);
    for _ in 0..count {
        values.push(i16::from_le_bytes([payload[*cursor], payload[*cursor + 1]]));
        *cursor += 2;
    }
    values
}

#[inline]
fn read_u16(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([bytes[offset], bytes[offset + 1]])
}

#[inline]
fn read_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
    ])
}

#[inline]
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
