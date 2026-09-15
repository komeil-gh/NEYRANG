# SHEGERD Policy Phase Hoisting

## Hypothesis

The loaded additive policy recomputes the same material-phase bucket for every
move scored in one MovePicker. Computing that node-invariant bucket once per
picker should preserve every ordering decision while removing repeated
bitboard population counts.

## Boundary and gate

H4d stores one policy-phase integer in the existing MovePicker and passes it to
the unchanged six-family scorer. The value is computed once when the picker
first advances beyond its preferred move. No policy weight, stage, SEE result,
search rule, artifact, or UCI behavior changes; an absent policy still returns
zero without evaluating material phase.

Require all Rust tests, the tactical suite, Clippy, and identical hybrid
depth-5/depth-8 nodes, scores, best moves, and checksums. Measure at least 21
interleaved depth-8 hybrid runs per binary. Retain only if median wall time
improves by at least 1% without an anomaly.
