# REKHNE H5d: PV-only internal iterative reduction

## Hypothesis

H3v reduced unseeded non-PV branches and narrowly missed its parent gate. The
remaining distinct case is an unseeded principal-variation node: at depth six
or greater, no TT or caller move means the node lacks an ordering anchor, so
spending the full nominal depth before any line is established can be wasteful.
Current Stockfish also reduces sufficiently deep PV/cut nodes without a TT move:
<https://github.com/official-stockfish/Stockfish/blob/master/src/search.cpp>.

## Candidate

Reduce the effective depth by one only at non-root PV nodes of depth six or
greater that are outside check and synthetic null subtrees and have neither a
TT move nor a caller-provided move. All legal-terminal handling, mate-distance
bounds, SANJ, SHEGERD, pruning margins, move stages, artifacts, time controls,
and UCI defaults remain unchanged. This is deliberately narrower than both
Stockfish's tuned condition and rejected H3v.

## Gates

1. A focused boundary test, all Rust tests, the tactical suite, start-position
   perft 5, formatting, and Clippy must pass.
2. The five-position hybrid depth-8 tree must shrink by at least 2% without a
   solved tactical regression; every changed move or score is recorded.
3. A passing local gate opens fresh independent 256-game fixed-node and
   `0.5+0.005` parent screens against H4m. Both require non-negative point
   estimates and zero anomalies.
4. Only both passing parent screens open a fresh Blunder 7.6.0 screen, which
   must exceed the retained 26.56% point estimate.

Failure removes the playing code while retaining the measured outcome.

## Result

The focused boundary regression, all 143 Rust tests, the 11-position tactical
suite, start-position perft 5 (4,865,609 nodes), formatting, and Clippy passed.
The five-position hybrid depth-8 workload was nevertheless exactly inert:
H5d and H4m both searched 513,582 nodes with identical move/score signatures.

Iterative deepening and TT reuse supplied a preferred move at every qualifying
PV node in this workload. H5d therefore failed the registered 2% tree floor;
no remote games ran and the playing code was removed.
