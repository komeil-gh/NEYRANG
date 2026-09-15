# REKHNE H4y: score-centered aspiration recovery

## Hypothesis

NEYRANG widens a failed root aspiration window from the stale window edge.
When a fail-soft result lands far outside that window, geometric edge widening
can repeat searches that already know a better center. Current Stockfish instead
repositions the failed side around the returned score while preserving a narrow
opposite edge:
<https://github.com/official-stockfish/Stockfish/blob/master/src/search.cpp>.

## Candidate

Keep NEYRANG's retained 50-centipawn initial delta and doubling schedule. On a
fail-low, set the upper edge to the old alpha and the lower edge to
`score - delta`. On a fail-high, keep a narrow lower edge below the old beta and
set the upper edge to `score + delta`. Clamp every edge to the existing infinite
bounds. No evaluator, move ordering, pruning, reduction, artifact, or UCI option
changes.

## Gates

1. A focused pure-function regression covers fail-low, fail-high, and bound
   clamping. All Rust tests, the 11-position tactical suite, start-position
   perft 5, formatting, and Clippy must pass.
2. All five hybrid depth-8 best-move/score signatures must remain exact and the
   combined tree may not grow. If it shrinks by at least 1%, continue as a
   search change; otherwise require a 21-run interleaved median speed gain of at
   least 1% for an identity-preserving throughput change.
3. A search change opens independent fresh 256-game fixed-node and equal-time
   parent screens against H4m, each requiring a non-negative point estimate and
   zero anomaly. A pure throughput result needs no game claim.

Failure removes the playing code while retaining the measured outcome.

## Result

The focused fail-low/fail-high/clamping regression passed. The candidate
preserved the complete five-position hybrid depth-8 identity at 513,582 nodes
and the exact five best-move/score signatures, so it did not qualify as a search
change. Across 21 interleaved runs per frozen binary, parent median elapsed time
was 0.461213 seconds and candidate median was 0.469116 seconds (-1.69%
throughput). It failed the registered identity-preserving speed gate; no remote
build or match was opened, and the playing code plus temporary test were
removed.
