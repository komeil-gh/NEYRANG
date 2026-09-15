# SANJ H5c: rule-50 conversion damping

## Hypothesis

REKHNE returns a draw at the rule-50 boundary, but SANJ values the same material
advantage identically at halfmove clocks 0 and 99. Near the boundary, this
overstates advantages that have little time left to convert and feeds optimistic
static scores into pruning. Current Stockfish explicitly damps evaluation as
the rule-50 counter rises:
<https://github.com/official-stockfish/Stockfish/blob/master/src/evaluate.cpp>.

## Candidate

After tapered white-relative SANJ is complete, multiply it by
`(199 - min(halfmove_clock, 100)) / 199` before side-to-move conversion and the
unchanged tempo bonus. This preserves clock-zero evaluation exactly, retains
half the positional score near the automatic-draw boundary, and leaves material,
PSQT, NNUE artifact, `EvalMix=10`, search, ordering, repetition, and terminal
draw rules unchanged.

## Gates

1. Focused tests cover clock zero, monotonic damping, color symmetry, and the
   capped clock. SANJ trace must reconstruct production exactly. All Rust tests,
   the tactical suite, perft 5, formatting, and Clippy must pass.
2. Record hybrid depth-8 nodes and every changed move/score. Reject if the tree
   grows by more than 20% or a solved tactical fixture regresses.
3. A passing local gate opens fresh independent 256-game fixed-node and
   `0.5+0.005` parent screens against H4m. Both require non-negative point
   estimates and zero anomalies before a fresh Blunder screen.

Failure removes the playing code while retaining the measured outcome.
