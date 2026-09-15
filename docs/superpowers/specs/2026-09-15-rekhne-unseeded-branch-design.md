# REKHNE Unseeded Branch Reduction

## Hypothesis

At a deep non-PV node with no TT/PV move, REKHNE has no evidence that the
current nominal depth is worth searching before move ordering establishes a
principal candidate. Reducing only that unseeded branch by one ply can spend
the saved nodes on branches already supported by transposition evidence.

## Boundary and gate

H3v builds on retained H3o. It reduces one ply only at non-root, non-PV,
non-check, non-null-subtree nodes of depth five or greater when neither the TT
nor the caller provides a preferred move. Null-move eligibility is evaluated
at the original depth; SANJ, SHEGERD, qsearch, ordering, history, extensions,
and all pruning margins remain unchanged.

Require all Rust tests and Clippy, at least a 2% reduction in the frozen
depth-8 hybrid tree, and no tactical-suite regression. A passing tree gate
opens 128 fresh paired openings at 20,000 nodes against frozen H3o with eight
match workers. Reject on a negative candidate point estimate or any candidate
anomaly; only a pass permits a separate fresh Blunder 7.6.0 screen.

## Tree gate

All 142 Rust tests, the 11-test tactical suite, and Clippy passed. With the
frozen N7 network, stage-aligned policy, and `EvalMix=10`, H3v reduced the
five-position depth-8 tree from 513,582 to 488,990 nodes (-4.79%) while
preserving all five best moves and reported scores. The clean 256-game parent
screen scored 79 wins, 81 losses, and 96 draws (49.61%, -2.71 +/- 20.62 Elo).
H3v failed the pre-registered non-negative point-estimate floor, so no Blunder
screen was opened and the playing code was removed.
