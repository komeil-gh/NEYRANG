# SANJ H13: exact pawn-structure cache

## Trigger evidence

Default evaluation still computes classical SANJ on every evaluated node, and
its pawn term depends only on the two pawn bitboards. Most ordinary moves leave
that structure unchanged. H13 targets repeated work without changing a score,
weight, search rule, network output, or UCI option.

## Candidate

Add one search-local direct-mapped table with 4,096 entries. Each entry stores
both complete pawn bitboards and the corresponding white-relative middlegame
and endgame terms. A lookup is accepted only when both bitboards match exactly,
so index collisions cannot change evaluation. The cache survives iterative
deepening and is private to each searcher.

The uncached SANJ API remains the reference path. Default 10% residual SANJ
uses the cached classical pawn component and the unchanged incremental N7
output.

## Gates

1. A focused cache/reference regression, formatting, Clippy with warnings
   denied, all-feature Rust tests, the 100,000-position SANJ trace oracle,
   tactical suite, perft, and UCI tests must pass on SSH.
2. The host-native depth-8 tree, best moves, scores, and checksum must remain
   exact. Across 21 interleaved pairs, retain H13 only for at least 2% lower
   median wall time than H12.
3. A passing engineering gate opens 1,000 clean paired `0.5+0.005` games
   against H12. A positive point estimate is required before any Blunder
   screen.

## Outcome

Rejected at the parent game gate. The focused regression, complete all-feature
test suite, 100,000-position SANJ trace and SEE oracles, tactical suite, perft,
formatting, and Clippy passed on SSH. H13 preserved the exact 536,259-node
depth-8 tree and `9d8d22b14e51010d` checksum. Across 21 interleaved host-native
pairs its median fell from 303 ms to 287 ms, a 5.57% throughput improvement.

The independently audited 1,000-game parent match nevertheless scored 49.40%
against H12 (`299/390/311`, `-4.17 +/-15.98 Elo`). All games terminated
normally with no warning, timeout, crash, legality, or protocol anomaly. The
negative point estimate failed the registered gate, so no Blunder screen was
opened and the playing cache was removed.
