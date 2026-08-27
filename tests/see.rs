use neyrang::{chess::Position, search::see};

#[test]
fn see_distinguishes_winning_and_losing_captures() {
    let mut winning = Position::from_fen("4k3/8/2p5/3q4/4P3/8/8/4K3 w - - 0 1")
        .expect("winning capture fixture is valid");
    let pawn_takes_queen = winning
        .find_legal_move("e4d5")
        .expect("pawn capture is legal");
    assert!(see(&winning, pawn_takes_queen) > 700);

    let mut losing = Position::from_fen("4k3/8/2p5/3p4/4Q3/8/8/4K3 w - - 0 1")
        .expect("losing capture fixture is valid");
    let queen_takes_pawn = losing
        .find_legal_move("e4d5")
        .expect("queen capture is legal");
    assert!(see(&losing, queen_takes_pawn) < -700);
}
