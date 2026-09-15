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
