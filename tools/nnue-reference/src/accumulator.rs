use neyrang::chess::{Color, PieceType, Position, Square};

use crate::{FeatureSet, HIDDEN_SIZE, Network, feature_index_for};

/// Both board-oriented hidden accumulators used by the scalar oracle.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AccumulatorPair {
    white: [i32; HIDDEN_SIZE],
    black: [i32; HIDDEN_SIZE],
    pub(crate) piece_count: u8,
}

impl AccumulatorPair {
    /// Rebuild both accumulators from the board.
    #[must_use]
    pub fn refresh(position: &Position, network: &Network) -> Self {
        let mut pair = Self {
            white: bias_as_i32(&network.feature_bias),
            black: bias_as_i32(&network.feature_bias),
            piece_count: position.all_occupancy().count_ones() as u8,
        };

        for index in 0_u8..64 {
            let square = Square::from_index(index).expect("board index is in range");
            if let Some((color, piece_type)) = position.piece_at(square) {
                pair.add_piece(position, color, piece_type, square, network);
            }
        }
        pair
    }

    /// Apply the exact piece-square difference between two legal board states.
    pub fn update(&mut self, before: &Position, after: &Position, network: &Network) {
        if matches!(
            network.feature_set,
            FeatureSet::Chess768KingBucketsMirrored3
                | FeatureSet::Chess768KingBucketsMirrored3PhaseHeads4
        ) && [Color::White, Color::Black].into_iter().any(|color| {
            before.pieces(color, PieceType::King) != after.pieces(color, PieceType::King)
        }) {
            *self = Self::refresh(after, network);
            return;
        }
        self.piece_count = after.all_occupancy().count_ones() as u8;
        for index in 0_u8..64 {
            let square = Square::from_index(index).expect("board index is in range");
            let old_piece = before.piece_at(square);
            let new_piece = after.piece_at(square);
            if old_piece == new_piece {
                continue;
            }
            if let Some((color, piece_type)) = old_piece {
                self.remove_piece(before, color, piece_type, square, network);
            }
            if let Some((color, piece_type)) = new_piece {
                self.add_piece(after, color, piece_type, square, network);
            }
        }
    }

    pub(crate) fn oriented(
        &self,
        side_to_move: Color,
    ) -> (&[i32; HIDDEN_SIZE], &[i32; HIDDEN_SIZE]) {
        match side_to_move {
            Color::White => (&self.white, &self.black),
            Color::Black => (&self.black, &self.white),
        }
    }

    /// Minimum and maximum hidden values across both perspectives.
    #[must_use]
    pub fn range(&self) -> (i32, i32) {
        self.white
            .iter()
            .chain(&self.black)
            .fold((i32::MAX, i32::MIN), |(minimum, maximum), &value| {
                (minimum.min(value), maximum.max(value))
            })
    }

    fn add_piece(
        &mut self,
        position: &Position,
        color: Color,
        piece_type: PieceType,
        square: Square,
        network: &Network,
    ) {
        add_feature(
            &mut self.white,
            feature_index_for(
                position,
                color,
                piece_type,
                square,
                Color::White,
                network.feature_set,
            ),
            network,
        );
        add_feature(
            &mut self.black,
            feature_index_for(
                position,
                color,
                piece_type,
                square,
                Color::Black,
                network.feature_set,
            ),
            network,
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
        remove_feature(
            &mut self.white,
            feature_index_for(
                position,
                color,
                piece_type,
                square,
                Color::White,
                network.feature_set,
            ),
            network,
        );
        remove_feature(
            &mut self.black,
            feature_index_for(
                position,
                color,
                piece_type,
                square,
                Color::Black,
                network.feature_set,
            ),
            network,
        );
    }
}

fn bias_as_i32(bias: &[i16]) -> [i32; HIDDEN_SIZE] {
    let mut values = [0; HIDDEN_SIZE];
    for (target, &source) in values.iter_mut().zip(bias) {
        *target = i32::from(source);
    }
    values
}

fn add_feature(accumulator: &mut [i32; HIDDEN_SIZE], feature: usize, network: &Network) {
    let start = feature * HIDDEN_SIZE;
    for (target, &weight) in accumulator
        .iter_mut()
        .zip(&network.feature_weights[start..start + HIDDEN_SIZE])
    {
        *target += i32::from(weight);
    }
}

fn remove_feature(accumulator: &mut [i32; HIDDEN_SIZE], feature: usize, network: &Network) {
    let start = feature * HIDDEN_SIZE;
    for (target, &weight) in accumulator
        .iter_mut()
        .zip(&network.feature_weights[start..start + HIDDEN_SIZE])
    {
        *target -= i32::from(weight);
    }
}
