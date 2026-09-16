# REKHNE H10: four-entry transposition clusters

## Trigger evidence

The retained table stores one entry per index even though each local entry is
16 bytes. Four local entries therefore fit one 64-byte cache line. In the
12,120 usable Stockfish-18-labelled decisions from the Blunder forensics,
100-centipawn errors fell from 15.01% at completed depth five to 2.62% at depth
eight. H10 targets effective depth by retaining more colliding transpositions;
it does not add pruning, extensions, evaluation terms, or learned weights.

## Candidate

Keep the exact Hash memory budget and group existing entries four at a time.
Probe the complete cluster. Store into the matching, empty, or stale slot;
otherwise replace only the cluster's least valuable current-generation entry
when the incoming depth plus exact-bound bonus is at least as valuable. Apply
the same cluster contract to local and packed shared tables without changing
entry layouts, score normalization, generation semantics, or atomic ordering.

Expose the already-collected TT hit and cutoff counters in the opt-in `stats`
benchmark output. This diagnostic has no effect on ordinary builds.

## Gates

1. Formatting, Clippy with warnings denied, all-feature Rust tests, the
   100,000-position move oracle, SANJ trace oracle, SEE oracle, tactical tests,
   memory-budget tests, and concurrent shared-TT coherence must pass on SSH.
2. A deterministic host-native depth-8 comparison must show no node increase
   and either at least 1% fewer nodes or 3% lower 21-pair median wall time.
   TT hits and cutoffs are recorded diagnostically, not used alone to accept.
3. A clean 1,000-game 20,000-node paired screen against H9 must have a positive
   point estimate before equal-time testing.
4. A clean 1,000-game `0.5+0.005` paired screen against H9 must also have a
   positive point estimate before the same-opening Blunder screen.
5. External progress requires a clean 1,000-game Blunder 7.6.0 score above the
   retained H9 baseline of 28.10%. Confidence intervals remain mandatory; a
   positive point estimate is not described as statistically proven when its
   interval crosses zero.

## Outcome

Rejected at the engineering gate before games.

The complete SSH test suite passed, including the new local collision test and
the existing shared-table memory, generation, mate-normalization, and
concurrency checks. The candidate increased deterministic depth-8 TT hits from
32,183 to 32,229 and TT cutoffs from 8,027 to 8,033, but reduced the tree only
from 536,259 to 536,228 nodes (0.006%). Across 21 interleaved host-native pairs,
both H9 and H10 had the same 295 ms median. This missed both registered
engineering floors, so no fixed-node, equal-time, or Blunder game was run and
all clustered-TT playing code was removed.
