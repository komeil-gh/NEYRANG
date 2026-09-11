//! Frozen additive move-ordering policy learned in SHEGERD-P1.

use crate::chess::{Move, PieceType, Position, Square};

use super::fischer::{material_phase, normalized_square};

const MAGIC: &[u8; 8] = b"NYRSHGP1";
const VERSION: u32 = 1;
const HEADER_LEN: usize = 36;
const FAMILY_SIZES: [usize; 6] = [64 * 64, 6 * 64, 7 * 7, 3, 65 * 64, 5];
const FAMILY_OFFSETS: [usize; 6] = [0, 4096, 4480, 4529, 4532, 8692];
const WEIGHTS: usize = 8697;
const MODEL_LEN: usize = HEADER_LEN + 2 * WEIGHTS;
const MODEL: &[u8; MODEL_LEN] = include_bytes!("neyrang-shegerd-policy-v1.bin");

const fn u32_at(offset: usize) -> u32 {
    u32::from_le_bytes([
        MODEL[offset],
        MODEL[offset + 1],
        MODEL[offset + 2],
        MODEL[offset + 3],
    ])
}

const fn valid_model() -> bool {
    let mut index = 0;
    while index < MAGIC.len() {
        if MODEL[index] != MAGIC[index] {
            return false;
        }
        index += 1;
    }
    if u32_at(8) != VERSION {
        return false;
    }
    index = 0;
    while index < FAMILY_SIZES.len() {
        if u32_at(12 + 4 * index) as usize != FAMILY_SIZES[index] {
            return false;
        }
        index += 1;
    }
    true
}

const _: () = assert!(valid_model(), "embedded SHEGERD policy is invalid");

#[inline]
fn weight(family: usize, feature: usize) -> i32 {
    let index = FAMILY_OFFSETS[family] + feature;
    let offset = HEADER_LEN + 2 * index;
    i16::from_le_bytes([MODEL[offset], MODEL[offset + 1]]) as i32
}

#[inline]
const fn see_bucket(exchange: i32) -> usize {
    if exchange <= -100 {
        0
    } else if exchange < 0 {
        1
    } else if exchange == 0 {
        2
    } else if exchange < 100 {
        3
    } else {
        4
    }
}

#[inline]
pub(crate) fn score(
    position: &Position,
    mv: Move,
    previous_to: Option<Square>,
    exchange: i32,
) -> i32 {
    let color = position.side_to_move();
    let from = normalized_square(mv.from().index(), color);
    let to = normalized_square(mv.to().index(), color);
    let mover = position
        .piece_at(mv.from())
        .expect("a legal move must have a source piece")
        .1
        .index();
    let victim = if mv.is_en_passant() {
        PieceType::Pawn.index() + 1
    } else {
        position
            .piece_at(mv.to())
            .map_or(0, |(_, piece)| piece.index() + 1)
    };
    let promotion = mv.promotion().map_or(0, |piece| piece.index() + 1);
    let previous = previous_to.map_or(0, |square| normalized_square(square.index(), color) + 1);
    let features = [
        from * 64 + to,
        mover * 64 + to,
        victim * 7 + promotion,
        material_phase(position),
        previous * 64 + to,
        see_bucket(exchange),
    ];

    features
        .into_iter()
        .enumerate()
        .fold(0_i32, |total, (family, feature)| {
            total.saturating_add(weight(family, feature))
        })
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use crate::chess::{Position, Square};

    use super::{MODEL, MODEL_LEN, score, valid_model};

    #[test]
    fn embedded_artifact_has_the_registered_shape() {
        assert_eq!(MODEL.len(), MODEL_LEN);
        assert!(valid_model());
    }

    #[test]
    fn score_is_deterministic_for_present_and_absent_context() {
        let mut position = Position::startpos();
        let mv = position.find_legal_move("e2e4").unwrap();
        let previous = Square::from_str("e5").unwrap();
        assert_eq!(score(&position, mv, None, 0), score(&position, mv, None, 0));
        assert_eq!(
            score(&position, mv, Some(previous), 0),
            score(&position, mv, Some(previous), 0)
        );
    }

    #[test]
    fn scores_match_the_independent_public_fixture() {
        let fixture = include_str!("../../docs/evidence/shegerd-p2-score-parity.tsv");
        for (line_number, line) in fixture.lines().skip(1).enumerate() {
            let fields: Vec<_> = line.split('\t').collect();
            assert_eq!(fields.len(), 6, "fixture line {}", line_number + 2);
            let mut position = Position::from_fen(fields[1]).unwrap();
            let mv = position.find_legal_move(fields[2]).unwrap();
            let previous = if fields[3] == "-" {
                None
            } else {
                Some(Square::from_str(fields[3]).unwrap())
            };
            let exchange = fields[4].parse().unwrap();
            let expected = fields[5].parse().unwrap();
            assert_eq!(
                score(&position, mv, previous, exchange),
                expected,
                "fixture case {}",
                fields[0]
            );
        }
    }
}
