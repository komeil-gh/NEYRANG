//! Quantized additive move policy used only inside existing MovePicker stages.

use std::fmt;

use crate::chess::{Color, Move, PieceType, Position, Square};

const MAGIC: &[u8; 8] = b"NYRSHGP1";
const VERSION: u32 = 2;
const FAMILY_SIZES: [usize; 6] = [64 * 64, 6 * 64, 7 * 7, 3, 65 * 64, 5];
const OFFSETS: [usize; 6] = [0, 4096, 4480, 4529, 4532, 8692];
const WEIGHT_COUNT: usize = 8697;
const HEADER_SIZE: usize = 8 + 8 * 4;
const MAX_ABS_WEIGHT: i16 = 128;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Network {
    weights: Box<[i16]>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NetworkError {
    BadMagic,
    UnsupportedVersion(u32),
    DimensionMismatch,
    LengthMismatch { expected: usize, actual: usize },
    ChecksumMismatch { expected: u32, actual: u32 },
    WeightOutOfRange { index: usize, value: i16 },
}

impl fmt::Display for NetworkError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BadMagic => formatter.write_str("invalid SHEGERD policy magic"),
            Self::UnsupportedVersion(version) => {
                write!(formatter, "unsupported SHEGERD policy version {version}")
            }
            Self::DimensionMismatch => {
                formatter.write_str("SHEGERD policy table dimensions differ")
            }
            Self::LengthMismatch { expected, actual } => {
                write!(formatter, "policy has {actual} bytes; expected {expected}")
            }
            Self::ChecksumMismatch { expected, actual } => write!(
                formatter,
                "policy payload checksum {actual:08x} does not match {expected:08x}"
            ),
            Self::WeightOutOfRange { index, value } => write!(
                formatter,
                "policy weight {index} is {value}; expected within +/-{MAX_ABS_WEIGHT}"
            ),
        }
    }
}

impl std::error::Error for NetworkError {}

impl Network {
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, NetworkError> {
        let expected_length = HEADER_SIZE + WEIGHT_COUNT * 2;
        if bytes.len() != expected_length {
            return Err(NetworkError::LengthMismatch {
                expected: expected_length,
                actual: bytes.len(),
            });
        }
        if &bytes[..MAGIC.len()] != MAGIC {
            return Err(NetworkError::BadMagic);
        }
        let version = read_u32(bytes, 8);
        if version != VERSION {
            return Err(NetworkError::UnsupportedVersion(version));
        }
        for (index, &expected) in FAMILY_SIZES.iter().enumerate() {
            if read_u32(bytes, 12 + index * 4) as usize != expected {
                return Err(NetworkError::DimensionMismatch);
            }
        }
        let payload = &bytes[HEADER_SIZE..];
        let expected_checksum = read_u32(bytes, 36);
        let actual_checksum = crc32(payload);
        if actual_checksum != expected_checksum {
            return Err(NetworkError::ChecksumMismatch {
                expected: expected_checksum,
                actual: actual_checksum,
            });
        }
        let mut weights = Vec::with_capacity(WEIGHT_COUNT);
        let (pairs, remainder) = payload.as_chunks::<2>();
        debug_assert!(remainder.is_empty());
        for (index, bytes) in pairs.iter().enumerate() {
            let value = i16::from_le_bytes([bytes[0], bytes[1]]);
            if !(-MAX_ABS_WEIGHT..=MAX_ABS_WEIGHT).contains(&value) {
                return Err(NetworkError::WeightOutOfRange { index, value });
            }
            weights.push(value);
        }
        Ok(Self {
            weights: weights.into_boxed_slice(),
        })
    }

    #[cfg(test)]
    pub(crate) fn from_test_weights(weights: Vec<i16>) -> Self {
        assert_eq!(weights.len(), WEIGHT_COUNT);
        assert!(
            weights
                .iter()
                .all(|weight| (-MAX_ABS_WEIGHT..=MAX_ABS_WEIGHT).contains(weight))
        );
        Self {
            weights: weights.into_boxed_slice(),
        }
    }

    #[inline]
    pub(crate) fn score(
        &self,
        position: &Position,
        mv: Move,
        previous_to: Option<Square>,
        exchange: i32,
    ) -> i32 {
        let color = position.side_to_move();
        let from = normalized(mv.from(), color);
        let to = normalized(mv.to(), color);
        let mover = position
            .piece_at(mv.from())
            .map(|(_, piece)| piece.index())
            .expect("a legal policy move must have a mover");
        let victim = if mv.is_en_passant() {
            piece_code(PieceType::Pawn)
        } else {
            position
                .piece_at(mv.to())
                .map_or(0, |(_, piece)| piece_code(piece))
        };
        let promotion = mv.promotion().map_or(0, piece_code);
        let previous = previous_to.map_or(0, |square| normalized(square, color) + 1);
        let see = if mv.is_capture() || mv.is_promotion() {
            see_bucket(exchange)
        } else {
            2
        };
        let indices = [
            OFFSETS[0] + from * 64 + to,
            OFFSETS[1] + mover * 64 + to,
            OFFSETS[2] + victim * 7 + promotion,
            OFFSETS[3] + phase(position),
            OFFSETS[4] + previous * 64 + to,
            OFFSETS[5] + see,
        ];
        indices
            .into_iter()
            .map(|index| i32::from(self.weights[index]))
            .sum()
    }
}

#[inline]
const fn normalized(square: Square, color: Color) -> usize {
    match color {
        Color::White => square.index(),
        Color::Black => square.index() ^ 56,
    }
}

#[inline]
const fn piece_code(piece: PieceType) -> usize {
    piece.index() + 1
}

fn phase(position: &Position) -> usize {
    let non_pawn = [
        (PieceType::Knight, 3),
        (PieceType::Bishop, 3),
        (PieceType::Rook, 5),
        (PieceType::Queen, 9),
    ]
    .into_iter()
    .map(|(piece, value)| {
        value
            * (position.pieces(Color::White, piece).count_ones()
                + position.pieces(Color::Black, piece).count_ones())
    })
    .sum::<u32>();
    if non_pawn >= 52 {
        0
    } else if non_pawn >= 24 {
        1
    } else {
        2
    }
}

#[inline]
const fn see_bucket(value: i32) -> usize {
    if value <= -100 {
        0
    } else if value < 0 {
        1
    } else if value == 0 {
        2
    } else if value < 100 {
        3
    } else {
        4
    }
}

fn read_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(
        bytes[offset..offset + 4]
            .try_into()
            .expect("checked length"),
    )
}

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = u32::MAX;
    for &byte in bytes {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            let mask = 0_u32.wrapping_sub(crc & 1);
            crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
        }
    }
    !crc
}

#[cfg(test)]
mod tests {
    use super::{FAMILY_SIZES, HEADER_SIZE, MAGIC, Network, VERSION, WEIGHT_COUNT, crc32};
    use crate::chess::Position;

    fn artifact(weights: &[i16]) -> Vec<u8> {
        let mut payload = Vec::with_capacity(weights.len() * 2);
        for weight in weights {
            payload.extend_from_slice(&weight.to_le_bytes());
        }
        let mut bytes = Vec::with_capacity(HEADER_SIZE + payload.len());
        bytes.extend_from_slice(MAGIC);
        bytes.extend_from_slice(&VERSION.to_le_bytes());
        for size in FAMILY_SIZES {
            bytes.extend_from_slice(&(size as u32).to_le_bytes());
        }
        bytes.extend_from_slice(&crc32(&payload).to_le_bytes());
        bytes.extend_from_slice(&payload);
        bytes
    }

    #[test]
    fn scores_the_registered_additive_features() {
        let position = Position::startpos();
        let mv = position
            .clone()
            .find_legal_move("e2e4")
            .expect("fixture move is legal");
        let mut weights = vec![0_i16; WEIGHT_COUNT];
        for index in [796, 4124, 4480, 4529, 4560, 8694] {
            weights[index] = 7;
        }
        let network = Network::from_bytes(&artifact(&weights)).expect("artifact is valid");

        assert_eq!(network.score(&position, mv, None, 0), 42);
    }

    #[test]
    fn rejects_payload_tampering_and_unbounded_weights() {
        let mut weights = vec![0_i16; WEIGHT_COUNT];
        let mut bytes = artifact(&weights);
        *bytes.last_mut().expect("payload exists") ^= 1;
        assert!(Network::from_bytes(&bytes).is_err());

        weights[0] = 129;
        assert!(Network::from_bytes(&artifact(&weights)).is_err());

        weights[0] = i16::MIN;
        assert!(Network::from_bytes(&artifact(&weights)).is_err());
    }
}
