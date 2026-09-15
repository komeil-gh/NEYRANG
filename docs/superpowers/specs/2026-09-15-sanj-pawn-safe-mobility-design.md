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
