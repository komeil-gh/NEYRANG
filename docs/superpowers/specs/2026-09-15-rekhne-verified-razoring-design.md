# REKHNE Verified Razoring

## Hypothesis

At a shallow non-PV node whose static SANJ score is far below alpha, run the
existing quiescence search before the full main-search move loop. Return only
when qsearch independently confirms the fail-low. This can remove hopeless
quiet subtrees without trusting static evaluation alone.

## Boundary and gate

H3p is limited to depths one and two with margin `200 + 150 * depth`. Root, PV,
check, mate-window, low-material, and synthetic-null nodes are excluded. The
candidate builds on retained H3o and changes no evaluator, ordering table, or
other pruning margin. Require full tests, at least 2% depth-8 tree reduction,
then a 128-game 20,000-node paired screen against frozen H3o. Reject on a
negative point estimate or any candidate anomaly; only a pass permits Blunder.

## Outcome

The candidate grew the deterministic depth-8 tree from 536,259 nodes and
checksum `9d8d22b14e51010d` to 544,767 nodes and checksum
`03db69a6558155fb`, a 1.59% increase. H3p failed its pre-registered tree gate,
so no games ran and the playing code was removed.
