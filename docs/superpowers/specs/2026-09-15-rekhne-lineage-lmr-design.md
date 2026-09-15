# REKHNE H4i: lineage-tracked improving LMR

## Hypothesis

H4e's blanket second-ply reduction was fast but negative, while H4h proved that
a shared per-ply cache cannot identify the active ancestor without stale-value
risk. The correct minimal signal is immutable search-line context: carry the
parent and same-side grandparent SANJ scores with each recursive call, then use
the second reduction only when the current position has not improved.

## Candidate

Extend REKHNE's existing copyable `SearchContext` with parent and grandparent
static scores. Values shift only when descending the active line, so siblings
cannot leak into one another. Existing non-PV pruning already computes the
current score. At PV nodes of depth eight or greater only, compute one static
score so a depth-six descendant can receive a valid same-side comparison.

The ninth or later eligible quiet at depth six or greater receives H4e's second
reduction ply only when current SANJ is no higher than the active same-side
grandparent score and meaningful non-pawn material remains. Every H4c exclusion
and the normal-depth alpha-raise re-search remain authoritative. NNUE blend,
policy, other pruning margins, artifacts, time controls, and UCI defaults are
frozen.

## Gates

1. Full Rust tests, the tactical suite, start-position perft 5, and Clippy must
   pass, including a direct lineage-shift/isolation check.
2. The five-position hybrid depth-8 tree must shrink by at least 5% without
   changing any final best move or score.
3. A passing engineering gate opens 128 fresh, disjoint color-reversed pairs at
   20,000 nodes against H4c. Reject on an anomaly or negative point estimate.
4. Only a passing parent screen opens a separate 256-game Blunder 7.6.0 screen;
   it must exceed the retained 26.56% point estimate to claim progress.

## Engineering result

All 142 Rust tests, the 11-test tactical suite, start-position perft 5, and
Clippy passed. H4i preserved all five hybrid depth-8 best moves and scores while
reducing their combined tree from 513,582 to 412,324 nodes (-19.72%). Across 21
interleaved runs per binary, median time improved from 0.444238 to 0.372122
seconds (+19.38% throughput). The 20-position historical Blunder-error replay
also produced exactly the same move and completed-depth sequence as H4c.

## Match outcome

The fresh strict 256-game parent screen completed without warnings or stderr.
H4i scored 84 wins, 93 draws, and 79 losses: 50.98%
(`+6.79 +/-16.59 Elo`), so it opened the external gate.

The first Blunder run was interrupted after the immutable opponent emitted its
known post-threefold PV warning; it is not decision evidence. A complete rerun
of the same preselected 128 pairs disabled strict interruption and registered
the narrow opponent-warning audit policy before launch. It finished with zero
warnings, empty stderr, and 256 normal terminations. H4i scored 30 wins, 49
draws, and 177 losses: 21.29% (`-227.15 +/-43.59 Elo`), below the retained H3o
26.56% point estimate. The retained UTF-16 PowerShell log and PGN were
independently re-audited after log decoding was corrected; the audit returned
`ok: true` with totals and pentanomial counts matching fastchess. H4i therefore
failed its external gate and the playing code was removed.
