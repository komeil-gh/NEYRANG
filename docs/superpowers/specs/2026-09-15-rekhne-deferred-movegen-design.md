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
