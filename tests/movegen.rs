use std::mem::size_of;

use neyrang::chess::{Color, Position, attacks};
use neyrang::chess::{Move, MoveFlag, MoveList, PieceType, Square};

#[test]
fn compact_move_round_trips_its_encoded_fields() {
    let mv = Move::new(Square::E7, Square::E8, MoveFlag::QueenPromotionCapture);

    assert_eq!(size_of::<Move>(), 2);
    assert_eq!(mv.from(), Square::E7);
    assert_eq!(mv.to(), Square::E8);
    assert!(mv.is_capture());
    assert_eq!(mv.promotion(), Some(PieceType::Queen));
    assert_eq!(mv.to_string(), "e7e8q");
}

#[test]
fn fixed_move_list_iterates_without_heap_storage() {
    let first = Move::new(Square::E2, Square::E4, MoveFlag::DoublePawnPush);
    let second = Move::new(Square::G1, Square::F3, MoveFlag::Quiet);
    let mut moves = MoveList::new();

    moves.push(first);
    moves.push(second);

    assert_eq!(moves.len(), 2);
    assert_eq!(
        moves.iter().copied().collect::<Vec<_>>(),
        vec![first, second]
    );
}

#[test]
fn portable_attack_generation_respects_edges_and_blockers() {
    assert_eq!(attacks::knight_attacks(Square::A1).count_ones(), 2);
    assert_eq!(attacks::knight_attacks(Square::D4).count_ones(), 8);
    assert_eq!(
        attacks::pawn_attacks(Color::White, Square::A2),
        Square::B3.bit()
    );

    let blockers = Square::D2.bit() | Square::B4.bit() | Square::F4.bit() | Square::D6.bit();
    let expected = Square::D3.bit()
        | Square::D2.bit()
        | Square::C4.bit()
        | Square::B4.bit()
        | Square::E4.bit()
        | Square::F4.bit()
        | Square::D5.bit()
        | Square::D6.bit();
    assert_eq!(attacks::rook_attacks(Square::D4, blockers), expected);
}

#[test]
fn starting_position_has_twenty_legal_moves() {
    let mut position = Position::startpos();
    let moves = position.legal_moves();
    let notation: Vec<String> = moves.iter().map(ToString::to_string).collect();

    assert_eq!(moves.len(), 20);
    assert!(notation.contains(&"e2e4".to_owned()));
    assert!(notation.contains(&"g1f3".to_owned()));
}
