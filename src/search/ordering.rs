use crate::chess::{Color, Move, MoveList, PieceType, Position};

use super::{history::HistoryTable, see::see};

const PIECE_VALUE: [i32; 6] = [100, 320, 330, 500, 900, 20_000];
const TT_SCORE: i32 = 1_000_000;
const GOOD_TACTICAL_SCORE: i32 = 200_000;
const BAD_CAPTURE_SCORE: i32 = -100_000;

#[derive(Clone, Copy)]
struct ScoredMove {
    mv: Move,
    score: i32,
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct OrderingStatistics {
    #[cfg(feature = "stats")]
    pub see_calls: u64,
    #[cfg(feature = "stats")]
    pub good_captures: u64,
    #[cfg(feature = "stats")]
    pub bad_captures: u64,
}

pub(crate) fn order(
    position: &Position,
    moves: &mut MoveList,
    preferred: Option<Move>,
    killers: [Move; 2],
    history: &HistoryTable,
    color: Color,
) -> OrderingStatistics {
    let mut statistics = OrderingStatistics::default();
    let mut scored = [ScoredMove {
        mv: Move::NONE,
        score: i32::MIN,
    }; MoveList::CAPACITY];
    let move_count = moves.len();
    for (index, &mv) in moves.iter().enumerate() {
        scored[index] = ScoredMove {
            mv,
            score: score(
                position,
                mv,
                preferred,
                killers,
                history,
                color,
                &mut statistics,
            ),
        };
    }
    scored[..move_count].sort_unstable_by_key(|entry| std::cmp::Reverse(entry.score));
    for (destination, entry) in moves
        .as_mut_slice()
        .iter_mut()
        .zip(scored[..move_count].iter())
    {
        *destination = entry.mv;
    }
    statistics
}

fn score(
    position: &Position,
    mv: Move,
    preferred: Option<Move>,
    killers: [Move; 2],
    history: &HistoryTable,
    color: Color,
    _statistics: &mut OrderingStatistics,
) -> i32 {
    if preferred == Some(mv) {
        return TT_SCORE;
    }
    if mv.is_capture() || mv.is_promotion() {
        #[cfg(feature = "stats")]
        {
            _statistics.see_calls += 1;
        }
        let exchange = see(position, mv);
        let promotion_bonus = mv
            .promotion()
            .map_or(0, |promotion| PIECE_VALUE[promotion.index()] * 16);
        let victim = if mv.is_en_passant() {
            PieceType::Pawn
        } else {
            position
                .piece_at(mv.to())
                .map_or(PieceType::Pawn, |(_, kind)| kind)
        };
        let attacker = position
            .piece_at(mv.from())
            .map_or(PieceType::Pawn, |(_, kind)| kind);
        let mvv_lva = if mv.is_capture() {
            PIECE_VALUE[victim.index()] * 16 - PIECE_VALUE[attacker.index()]
        } else {
            0
        };
        if mv.is_promotion() || exchange >= 0 {
            #[cfg(feature = "stats")]
            if mv.is_capture() {
                _statistics.good_captures += 1;
            }
            return GOOD_TACTICAL_SCORE + promotion_bonus + mvv_lva + exchange;
        }
        #[cfg(feature = "stats")]
        {
            _statistics.bad_captures += 1;
        }
        return BAD_CAPTURE_SCORE + mvv_lva + exchange;
    }
    let mut score = 0;
    if mv.is_castle() {
        score += 500;
    }
    if killers[0] == mv {
        score += 90_000;
    } else if killers[1] == mv {
        score += 80_000;
    }
    score += history.score(color, mv);
    score
}

#[cfg(test)]
mod tests {
    use crate::chess::{Move, Position};

    use super::{HistoryTable, order};

    fn index_of(moves: &[Move], expected: Move) -> usize {
        moves
            .iter()
            .position(|&mv| mv == expected)
            .expect("expected move must remain in the ordered list")
    }

    #[test]
    fn orders_bad_captures_after_killers_and_quiets() {
        let mut position = Position::from_fen("6k1/8/5p2/3qp3/2P1Q3/8/8/6K1 w - - 0 1")
            .expect("ordering fixture must be valid");
        let preferred = position
            .find_legal_move("e4d3")
            .expect("preferred quiet move must be legal");
        let good_capture = position
            .find_legal_move("c4d5")
            .expect("winning capture must be legal");
        let bad_capture = position
            .find_legal_move("e4e5")
            .expect("losing capture must be legal");
        let killer = position
            .find_legal_move("g1f2")
            .expect("killer move must be legal");
        let quiet = position
            .find_legal_move("c4c5")
            .expect("ordinary quiet move must be legal");
        let mut moves = position.legal_moves();

        let ordering_statistics = order(
            &position,
            &mut moves,
            Some(preferred),
            [killer, Move::NONE],
            &HistoryTable::default(),
            position.side_to_move(),
        );

        let ordered = moves.as_slice();
        assert_eq!(ordered[0], preferred);
        assert!(index_of(ordered, good_capture) < index_of(ordered, killer));
        assert!(index_of(ordered, killer) < index_of(ordered, quiet));
        assert!(index_of(ordered, quiet) < index_of(ordered, bad_capture));
        #[cfg(feature = "stats")]
        {
            assert_eq!(ordering_statistics.see_calls, 3);
            assert_eq!(ordering_statistics.good_captures, 2);
            assert_eq!(ordering_statistics.bad_captures, 1);
        }
    }
}
