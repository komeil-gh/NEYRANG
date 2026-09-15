# REKHNE H4h: SANJ-conditioned deep LMR

## Hypothesis

H4e showed that a second reduction ply saves 20.40% of the hybrid depth-8 tree
but loses strength when applied to every late deep quiet. The missing signal is
whether the position is improving for the side to move. A late quiet in a node
whose SANJ score has not improved since the same side last moved is a narrower,
more defensible target for the extra reduction.

## Candidate

Retain the complete H4c search and its ordinary one-ply LMR. Cache only static
SANJ values that the existing pruning path already computes; do not add an
evaluation call. At depth six or greater, reduce the ninth or later eligible
quiet by a second ply only when the current static score is no higher than the
same side's score two plies earlier and the side has a major piece or at least
two minor pieces. Every H4c exclusion remains, and any reduced alpha raise is
re-searched at normal depth.

This adapts the improving signal used by current top alpha-beta searches to the
specific H4e failure instead of reviving its blanket reduction. SANJ weights,
NNUE blend, policy, pruning margins, artifacts, and UCI defaults remain frozen.

## Gates

1. Full Rust tests, the tactical suite, start-position perft 5, and Clippy must
   pass, including static-score stack reset and restoration checks.
2. The five-position hybrid depth-8 tree must shrink by at least 5% without
   changing any final best move or score.
3. A passing engineering gate opens 128 fresh, disjoint color-reversed pairs at
   20,000 nodes against H4c. Reject on an anomaly or negative point estimate.
4. Only a passing parent screen opens a separate 256-game Blunder 7.6.0 screen;
   it must exceed the retained 26.56% point estimate to claim progress.
