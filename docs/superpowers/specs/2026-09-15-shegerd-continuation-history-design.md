# SHEGERD Continuation History

## Hypothesis

Butterfly history merges the same quiet move across unrelated positions.
Remembering whether a quiet move succeeds after the previous piece-to-square
context can order plausible replies earlier and make existing LMR/LMP decisions
more trustworthy.

## Boundary and gate

H3z adds one 294,912-byte search-local table indexed by previous piece type and
destination, then current piece type and destination. It reuses the existing
bounded history update: a quiet beta-cutoff is rewarded and earlier searched
quiets are penalized. The score only orders quiets inside the current MovePicker
stage. Captures, SANJ, policy, pruning, reductions, and UCI defaults are unchanged.

Require all Rust tests, Clippy, the tactical suite, and no more than 10% growth
in the frozen depth-8 hybrid tree. Then run 128 fresh paired openings at 20,000
nodes against H3o. Reject on a negative point estimate or anomaly. Only a pass
opens a fresh 256-game Blunder 7.6.0 screen, which must exceed the retained
26.56% point estimate.

## Outcome

Rejected before performance or match testing. Both the full continuation
score and a quarter-weight ordering score broke the frozen mate-in-three
tactical regression at depth 6 while the retained H3o source passes it. The
candidate therefore failed the first correctness gate; no remote build or
games were run, and the playing source remains unchanged.
