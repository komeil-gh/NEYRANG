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

## Outcome

All 141 Rust tests, the 11-test tactical suite, and Clippy passed. H4a exactly
preserved the five-position hybrid tree at depth 5 (37,039 nodes) and depth 8
(513,582 nodes), including every final score and best move. Across 21
interleaved native runs per binary, median depth-8 wall time fell from
0.492566 seconds to 0.455146 seconds, an 8.222% throughput improvement. The
candidate passed the registered gate and is retained.

The target Windows node independently preserved the pure-N7 depth-8 identity
at 1,018,755 nodes and checksum `9f2a4572c73982f7`. Over 21 interleaved runs
per binary, median time fell from 1,158 ms to 840 ms, a 37.857% pure-N7
throughput improvement. The private evidence is retained under
`runs/h4a-native` on the test node.
