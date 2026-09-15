# REKHNE Tactical-Aware LMR

## Hypothesis

The retained LMR always removes one ply, even when a late quiet at depth four or
more is unlikely to matter. Remove two plies at those deeper nodes, but never
drop the probe below one main-search ply and never reduce a pawn push reaching
the sixth rank or beyond. Existing full-depth re-search remains authoritative
whenever the reduced probe raises alpha.

## Gate

H3q builds on retained H3o and changes only the reduced probe depth and the
advanced-pawn exemption. Require full tests and at least 10% depth-8 tree
reduction. Then run 64 paired openings at 20,000 nodes against frozen H3o with
identical policy and `EvalMix=10`. Reject on a negative point estimate or any
candidate anomaly; only a pass permits a separate Blunder 7.6 screen.

## Outcome

The first candidate reduced the deterministic depth-8 tree from 536,259 to
399,838 nodes, but failed the mate-in-three regression and was ineligible for
retention. Adding a low-material guard restored all 142 Rust tests and kept a
clean 128-game fixed-node parent score of 53/32/43 (58.20%,
`+57.52 +/-45.40 Elo`).

The amended candidate then scored only 28/163/65 (23.63%,
`-203.76 +/-42.83 Elo`) in its clean 256-game Blunder 7.6 screen. The only
warning was the registered immutable-Blunder threefold-PV event. Because the
required tactical repair erased the external point-estimate improvement, H3q
was rejected and its playing code removed.
