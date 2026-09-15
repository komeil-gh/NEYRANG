# SANJ Search-Local Pawn Table

## Hypothesis

Classical SANJ recomputes doubled, isolated, and passed-pawn terms at every
static evaluation even though many search nodes share the same two pawn
bitboards. A small search-local direct-mapped table can reuse those exact terms
without changing evaluation or introducing synchronization.

## Boundary and gate

H4b adds one 16,384-entry table per search worker. Each entry verifies both
complete pawn bitboards before returning the four cached tapered scores, so an
index collision is only a miss. The table is consulted only by REKHNE's static
evaluation path; standalone SANJ APIs, artifacts, search rules, UCI defaults,
and parallel ownership remain unchanged.

Require all Rust tests, the tactical suite, Clippy, and identical classical and
hybrid depth-5/depth-8 nodes, scores, best moves, and checksums. Measure at
least 21 interleaved depth-8 hybrid runs per binary. Retain only if median wall
time improves by at least 1% without an anomaly.

## Outcome

Rejected before remote benchmarking. All 142 Rust tests, the 11-test tactical
suite, and Clippy passed, and H4b preserved the complete depth-5/depth-8 hybrid
identity. Across 21 interleaved depth-8 runs per binary, however, median time
improved only from 0.454530 seconds to 0.451355 seconds (0.703%), below the
registered 1% floor. The cache and its search-worker state were removed.
