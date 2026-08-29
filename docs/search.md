# Search

## Current pipeline

NEYRANG uses iterative deepening over negamax alpha-beta. Non-first moves use a zero-window PVS search and are re-searched only when they improve alpha without failing high. From depth four onward, aspiration windows start at ±50 centipawns and widen geometrically on fail-low/high.

At depth zero, quiescence searches captures and promotions. Positions in check do not use stand pat and search every legal evasion. Mate scores encode root ply and are normalized when crossing the transposition-table boundary, so retrieval at a different ply preserves mate distance.

Move ordering is:

1. TT/PV move
2. promotions and MVV-LVA captures
3. killer moves
4. quiet history

SEE is independently implemented and tested, but a first attempt to call it inside the sort comparator was reverted: repeated SEE computation increased both nodes and wall time. A future integration should precompute a score once per move and be benchmarked again.

## Draws and limits

The search checks the 50-move counter and counts matching hashes within the reversible history window for threefold repetition. Maximum ply is 128. At the boundary it returns static evaluation rather than indexing past fixed search storage.

The UCI thread can set an atomic stop flag while search is active. Node limits are checked every node. Hard time is sampled every 1,024 nodes, while soft time is evaluated after completed iterations so NEYRANG always retains a legal move from the last stable iteration.

## Measured development changes

Single release runs on the same Apple Silicon host, five positions, depth 5:

| Revision step | Nodes | Time | Outcome |
| --- | ---: | ---: | --- |
| Alpha-beta + TT baseline | 1,060,548 | 832 ms | baseline |
| Killer + quiet history | 664,949 | 603 ms | retained |
| SEE inside comparator | 719,072 | 971 ms | reverted |
| PVS | 447,006 | 286 ms | retained |
| Aspiration ±25 | 477,104 | 317 ms | retuned |
| Aspiration ±50 | 448,136 | 282 ms median | retained |

The final row is the median of five runs; the preceding rows are single-run development snapshots. These are engineering measurements, not Elo evidence. Search-tree changes make raw NPS comparisons insufficient; paired games and SPRT decide strength patches.

## Next search work

- precomputed SEE ordering/pruning with tactical regressions
- conservative LMR behind tests and benchmark counters
- continuation/capture history after the basic history table has match evidence

Null move, futility, LMP, ProbCut, and singular extensions must be added one at a time rather than as a bundled selective-search rewrite.
