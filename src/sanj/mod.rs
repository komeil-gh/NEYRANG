//! SANJ: NEYRANG's replaceable position-judgment boundary.
//!
//! Classical SANJ remains the deterministic fallback. The default `nnue`
//! feature adds the retained residual evaluator behind the same search contract.

#[cfg(feature = "nnue")]
use std::sync::Arc;

#[cfg(feature = "nnue")]
use crate::chess::Move;
use crate::chess::Position;

mod classical;
#[cfg(feature = "nnue")]
pub mod nnue;
mod pawns;
mod psqt;
#[cfg(feature = "stockfish-nnue")]
mod stockfish;
#[cfg(any(test, feature = "sanj-tools"))]
mod trace;

pub use classical::{TEMPO, evaluate};
#[cfg(feature = "sanj-tools")]
pub use trace::{EvalTrace, TRACE_COLUMNS, TRACE_SCHEMA, trace};

/// Search-owned choice of SANJ implementation.
///
/// UCI chooses the embedded retained network in a default build. The enum keeps
/// classical as its construction default for explicit tools and tests.
#[derive(Clone, Default)]
pub enum Evaluator {
    #[default]
    Classical,
    #[cfg(feature = "nnue")]
    Nnue {
        network: Arc<nnue::Network>,
        nnue_percent: u8,
    },
    #[cfg(feature = "stockfish-nnue")]
    StockfishNnue {
        network: Arc<nnue_rs::Network>,
        nnue_percent: u8,
    },
}

#[cfg(feature = "nnue")]
#[derive(Clone)]
pub(crate) enum SearchAccumulator {
    Native(Box<nnue::AccumulatorPair>),
    #[cfg(feature = "stockfish-nnue")]
    Stockfish(nnue_rs::Accumulator),
}

impl Evaluator {
    #[must_use]
    pub const fn classical() -> Self {
        Self::Classical
    }

    #[cfg(feature = "nnue")]
    #[must_use]
    pub fn nnue(network: nnue::Network) -> Self {
        Self::nnue_with_mix(network, 100)
    }

    #[cfg(feature = "nnue")]
    #[must_use]
    pub fn nnue_with_mix(network: nnue::Network, nnue_percent: u8) -> Self {
        assert!(nnue_percent <= 100);
        Self::Nnue {
            network: Arc::new(network),
            nnue_percent,
        }
    }

    #[cfg(feature = "stockfish-nnue")]
    pub fn stockfish_nnue(bytes: &[u8]) -> Result<Self, nnue_rs::Error> {
        Ok(Self::StockfishNnue {
            network: Arc::new(nnue_rs::Network::from_bytes(bytes)?),
            nnue_percent: 100,
        })
    }

    #[must_use]
    pub fn evaluate(&self, position: &Position) -> i32 {
        match self {
            Self::Classical => evaluate(position),
            #[cfg(feature = "nnue")]
            Self::Nnue {
                network,
                nnue_percent,
            } => match nnue_percent {
                0 => evaluate(position),
                100 => network.evaluate(position),
                percent => blend(evaluate(position), network.evaluate(position), *percent),
            },
            #[cfg(feature = "stockfish-nnue")]
            Self::StockfishNnue {
                network,
                nnue_percent,
            } => match nnue_percent {
                0 => evaluate(position),
                100 => network.evaluate(position),
                percent => blend(evaluate(position), network.evaluate(position), *percent),
            },
        }
    }

    #[cfg(feature = "nnue")]
    pub fn set_nnue_mix(&mut self, nnue_percent: u8) {
        assert!(nnue_percent <= 100);
        if let Self::Nnue {
            nnue_percent: active,
            ..
        } = self
        {
            *active = nnue_percent;
        }
        #[cfg(feature = "stockfish-nnue")]
        if let Self::StockfishNnue {
            nnue_percent: active,
            ..
        } = self
        {
            *active = nnue_percent;
        }
    }

    #[cfg(feature = "nnue")]
    pub(crate) fn root_search_accumulator(&self, position: &Position) -> Option<SearchAccumulator> {
        match self {
            Self::Classical => None,
            Self::Nnue { network, .. } => Some(SearchAccumulator::Native(Box::new(
                nnue::AccumulatorPair::refresh(position, network),
            ))),
            #[cfg(feature = "stockfish-nnue")]
            Self::StockfishNnue { network, .. } => {
                Some(SearchAccumulator::Stockfish(network.accumulator(position)))
            }
        }
    }

    #[cfg(feature = "nnue")]
    pub(crate) fn empty_search_accumulator(&self) -> Option<SearchAccumulator> {
        match self {
            Self::Classical => None,
            Self::Nnue { network, .. } => Some(SearchAccumulator::Native(Box::new(
                nnue::AccumulatorPair::refresh(&Position::startpos(), network),
            ))),
            #[cfg(feature = "stockfish-nnue")]
            Self::StockfishNnue { network, .. } => {
                Some(SearchAccumulator::Stockfish(network.empty_accumulator()))
            }
        }
    }

    #[cfg(feature = "nnue")]
    pub(crate) fn update_search_accumulator(
        &self,
        position: &Position,
        mv: Move,
        parent: &SearchAccumulator,
        child: &mut SearchAccumulator,
    ) {
        match (self, parent, child) {
            (
                Self::Nnue { network, .. },
                SearchAccumulator::Native(parent),
                SearchAccumulator::Native(child),
            ) => **child = parent.after_move(position, mv, network),
            #[cfg(feature = "stockfish-nnue")]
            (
                Self::StockfishNnue { network, .. },
                SearchAccumulator::Stockfish(parent),
                SearchAccumulator::Stockfish(child),
            ) => {
                let mut next = position.clone();
                next.make_move(mv);
                network.update(position, &next, parent, child);
            }
            _ => unreachable!("evaluator and search accumulator must match"),
        }
    }

    #[cfg(feature = "nnue")]
    pub(crate) fn evaluate_search_accumulator(
        &self,
        position: &Position,
        accumulator: &SearchAccumulator,
    ) -> i32 {
        let (nnue_score, nnue_percent) = match (self, accumulator) {
            (
                Self::Nnue {
                    network,
                    nnue_percent,
                },
                SearchAccumulator::Native(accumulator),
            ) => (
                network.evaluate_accumulator(accumulator, position.side_to_move()),
                *nnue_percent,
            ),
            #[cfg(feature = "stockfish-nnue")]
            (
                Self::StockfishNnue {
                    network,
                    nnue_percent,
                },
                SearchAccumulator::Stockfish(accumulator),
            ) => (
                network.evaluate_accumulator(
                    accumulator,
                    stockfish::map_color(position.side_to_move()),
                ),
                *nnue_percent,
            ),
            _ => unreachable!("evaluator and search accumulator must match"),
        };
        match nnue_percent {
            0 => evaluate(position),
            100 => nnue_score,
            percent => blend(evaluate(position), nnue_score, percent),
        }
    }
}

#[cfg(feature = "nnue")]
fn blend(classical: i32, nnue: i32, nnue_percent: u8) -> i32 {
    let nnue_weight = i64::from(nnue_percent);
    ((i64::from(classical) * (100 - nnue_weight) + i64::from(nnue) * nnue_weight) / 100) as i32
}

#[cfg(all(test, feature = "nnue"))]
mod tests {
    use super::blend;

    #[test]
    fn bounded_mix_preserves_both_endpoints() {
        assert_eq!(blend(-300, 700, 0), -300);
        assert_eq!(blend(-300, 700, 25), -50);
        assert_eq!(blend(-300, 700, 100), 700);
    }

    #[cfg(feature = "stockfish-nnue")]
    #[test]
    fn stockfish_incremental_scores_match_full_refresh() {
        use super::Evaluator;
        use crate::chess::Position;

        let evaluator =
            Evaluator::stockfish_nnue(include_bytes!("../../assets/models/nn-37f18f62d772.nnue"))
                .expect("embedded network");
        let mut position = Position::startpos();
        let mut accumulator = evaluator
            .root_search_accumulator(&position)
            .expect("root accumulator");

        for notation in ["e2e4", "c7c5", "g1f3", "d7d6", "d2d4", "c5d4", "f3d4"] {
            let mv = position.find_legal_move(notation).expect("legal test move");
            let mut child = evaluator
                .empty_search_accumulator()
                .expect("child accumulator");
            evaluator.update_search_accumulator(&position, mv, &accumulator, &mut child);
            position.make_move(mv);

            assert_eq!(
                evaluator.evaluate_search_accumulator(&position, &child),
                evaluator.evaluate(&position)
            );
            accumulator = child;
        }
    }
}
