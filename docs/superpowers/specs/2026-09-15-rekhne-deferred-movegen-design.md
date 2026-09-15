# REKHNE Deferred Legal-Move Materialization

## Hypothesis

REKHNE currently builds the complete legal move list before reverse-futility
and null-move guards that often return without consuming it. Checking only for
the existence of one legal move at the actual cutoff boundary can preserve
terminal correctness while avoiding unused list construction.

## Boundary and gate

H4c leaves full legal generation in its existing place for every node that
continues to ordinary search. At a reverse-futility cutoff or before a
null-move probe, it uses the existing exact `has_legal_move` path to preserve
mate and stalemate handling, and only then defers full list construction until
after those guards. Evaluation, node counting, pruning conditions, ordering,
reductions, and UCI behavior are unchanged.

Require all Rust tests, perft, the tactical suite, Clippy, and identical
classical and hybrid depth-5/depth-8 nodes, scores, best moves, and checksums.
Measure at least 21 interleaved depth-8 hybrid runs per binary. Retain only if
median wall time improves by at least 1% without an anomaly.

## Outcome

All 141 Rust tests, the 11-test tactical suite, start-position perft 5, and
Clippy passed. H4c exactly preserved both classical and hybrid depth-5/depth-8
trees, scores, best moves, and checksums. Across 21 interleaved hybrid depth-8
runs per binary, median time fell from 0.454067 seconds to 0.443108 seconds, a
2.473% throughput improvement. The candidate passed the registered gate and is
retained.

The target Windows node independently preserved the pure-N7 depth-8 identity
at 1,018,755 nodes and checksum `9f2a4572c73982f7`. Across 21 interleaved runs
per binary, median time fell from 840 ms to 821 ms, a 2.314% improvement. The
private evidence is retained under `runs/h4c-native` on the test node.
