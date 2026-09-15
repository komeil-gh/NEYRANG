# REKHNE Deep Late-Move Reduction

## Hypothesis

The retained LMR removes exactly one ply from every eligible late quiet. At a
non-PV node of depth six or more, the ninth or later quiet has already survived
TT, tactical, killer, and history ordering; probing it two plies shallower can
save nodes while the existing full-depth re-search remains authoritative when
it raises alpha.

## Boundary and gate

H4e changes the reduction from one to two plies only at depth six or greater,
from searched-move index eight onward, and only when the side to move has a
major piece or at least two minor pieces. All existing LMR exclusions remain,
and every reduced alpha raise is re-searched at full depth. SANJ, ordering,
policy, pruning margins, artifacts, and UCI defaults are unchanged.

Require all Rust tests, perft, the tactical suite, Clippy, and at least 5%
reduction in the frozen hybrid depth-8 tree. A passing tree opens 128 fresh
paired openings at 20,000 nodes against H4c. Reject on a negative point
estimate or anomaly. Only a pass opens a separate 256-game Blunder 7.6.0
screen, which must exceed the retained 26.56% point estimate.
