# Search

## Current pipeline

NEYRANG uses iterative deepening over negamax alpha-beta. Non-first moves use a zero-window PVS search and are re-searched only when they improve alpha without failing high. From depth four onward, aspiration windows start at ±50 centipawns and widen geometrically on fail-low/high.

At depth zero, quiescence searches promotions and captures whose legal static exchange evaluation is non-negative. Positions in check do not use stand pat or SEE pruning and search every legal evasion. Mate scores encode root ply and are normalized when crossing the transposition-table boundary, so retrieval at a different ply preserves mate distance.

Move ordering is:

1. TT/PV move
2. promotions and non-losing captures, scored once with SEE plus MVV-LVA
3. killer moves
4. quiet history
5. losing captures

SEE updates temporary bitboards, exposes slider x-rays, handles promotions and en passant, excludes absolutely pinned attackers, and rejects illegal king recaptures. It is computed once per tactical move before sorting rather than from the comparator.

At non-root, non-PV nodes, sufficiently late quiet moves may be reduced by one ply. The reduction starts with the fifth searched move at depth three, excludes TT moves, killers, strong-history moves, captures, promotions, checks, and nodes already in check, and always performs a normal-depth zero-window re-search when the reduced result raises alpha. The ordinary PVS full-window re-search remains authoritative when needed.

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
| Precomputed SEE ordering | 528,816 | 464 ms median | retained after positive match screens |
| Qsearch SEE pruning | 359,538 | 203 ms median | retained |
| Conservative LMR | 196,627 | 166 ms median | retained and release-proven cumulatively |

The final row is the median of five runs; the preceding rows are single-run development snapshots. These are engineering measurements, not Elo evidence. Search-tree changes make raw NPS comparisons insufficient; paired games and SPRT decide strength patches.

## Experiment outcome and next search work

Guarded null-move pruning reduced this benchmark to 182,768 nodes but did not accept H1 in a capped 1,000-game SPRT against the LMR parent, so it was reverted. Reverse futility pruning was not attempted in 0.2.0.

Next candidates should be isolated and measured in this order:

- recover SEE/ordering throughput with a lazy threshold query and/or a staged MovePicker, without weakening the legal SEE oracle
- add Capture History as a separate experiment so tactical ordering learns from prior cutoffs in addition to material exchange safety
- add Continuation History as a separate experiment so quiet ordering gains previous-move context and gives LMR a better late-move distribution
- revisit guarded null-move pruning only against the resulting stronger ordering baseline

Pawn hashing remains useful but is deferred below these search-ordering experiments. Any renewed null move, reverse futility, LMP, ProbCut, or singular-extension work must be added one at a time rather than as a bundled selective-search rewrite. The detailed acceptance sequence is frozen in the [0.3.0 search plan](development/0.3.0-plan.md).
