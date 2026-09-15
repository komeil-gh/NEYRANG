# SANJ NNUE Hot-Loop Arithmetic Width

## Hypothesis

The 128-wide squared-clipped activation currently performs every multiply and
accumulate as `i128`, although the validated operands and complete dot product
fit in `i64`. Keeping only the final centipawn scaling in `i128` should preserve
every score and search decision while reducing NNUE inference cost.

## Boundary and gate

H4a changes only the activation dot product and its clipping helper from
`i128` to `i64`. The accumulated value is converted back to `i128` before the
existing bias, centipawn-scale multiplication, division, and final clamp, so
the artifact format and overflow behavior at the public boundary remain
unchanged. No search, ordering, evaluation weight, network, or UCI default is
changed.

Require all Rust tests, the tactical suite, Clippy, and identical depth-5 and
depth-8 hybrid nodes, scores, best moves, and checksums. Measure at least 21
interleaved depth-8 runs per binary on the native host. Retain only if median
wall time improves by at least 1% without an outlier or protocol anomaly.
