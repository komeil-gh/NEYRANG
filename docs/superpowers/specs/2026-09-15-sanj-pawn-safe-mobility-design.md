# SANJ H5b: pawn-safe minor mobility

## Hypothesis

SANJ currently rewards every geometrically reachable minor-piece square outside
friendly occupancy. A knight or bishop landing on a square already controlled
by an enemy pawn often does not have usable mobility, so the term can overvalue
exposed pieces. Established classical evaluators build mobility areas from
attack maps rather than raw reach alone; historical Stockfish evaluation code
also excludes unsafe squares before applying mobility bonuses:
<https://github.com/official-stockfish/Stockfish/blob/master/src/evaluate.cpp>.

## Candidate

Compute the enemy pawn attack map once per color evaluation and exclude those
squares from knight and bishop mobility. Keep rook and queen mobility, all
existing weights, PSQT, material, pawn structure, king danger, NNUE artifact,
`EvalMix=10`, search, ordering, and UCI defaults unchanged. This is a SANJ
feature correction, not a new parameter-tuning pass.

## Gates

1. A focused regression proves that an enemy pawn-controlled knight destination
   loses exactly one existing mobility unit. All Rust tests, the 11-position
   tactical suite, perft 5, formatting, and Clippy must pass.
2. Record every changed best move and score in the five-position hybrid depth-8
   workload. Reject locally if nodes grow by more than 20% or a solved tactical
   fixture regresses.
3. A passing local gate opens fresh independent 256-game fixed-node and
   `0.5+0.005` parent screens against H4m. Both require non-negative point
   estimates and zero anomalies before a fresh Blunder screen.

Failure removes the playing code while retaining the measured outcome.

## Result

The focused mobility regression, all 143 Rust tests, the 11-position tactical
suite, start-position perft 5 (4,865,609 nodes), formatting, and Clippy passed.
The five-position hybrid depth-8 tree fell from 513,582 to 479,651 nodes
(-6.61%). The equal-scored initial choice changed from `d2d4/23` to `g1f3/23`;
the other changed scores were `d7c8q/584` and `c3d5/116`.

The clean audited 20,000-node parent screen scored 86/86/84 over 256 games:
129/256 (50.39%), +2.71 +/-32.23 Elo. The separate clean audited equal-time
screen scored 76/99/81: 125.5/256 (49.02%), -6.79 +/-32.11 Elo. Both runs used
128 unique fresh color-reversed pairs and had zero crash, timeout, protocol,
forfeit, warning, or timing anomaly.

The equal-time point estimate is negative, so gate 3 fails. No Blunder screen
was opened and the playing code was removed. Private evidence is retained under
`testing/private/h5b-pawn-safe-mobility-20260915/`; the candidate executable
SHA-256 is
`570db1e72d3724ed8ce5f2206c081fc4c644a023461efb162b0b5ca44a3bdff1`.
