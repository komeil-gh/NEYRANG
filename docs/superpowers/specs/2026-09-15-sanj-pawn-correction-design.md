# SANJ Pawn-Structure Correction History

## Hypothesis

SANJ's static score has repeatable residual error for positions sharing the
same pawn skeleton. During one search, learn a small side-to-move correction
from completed quiet nodes and apply it to later static evaluations with the
same pawn key. Keep the correction bounded and search-local; it is neither a
new evaluator nor cross-game learning.

## Boundary and gate

H3t uses a 16,384-entry table per side, stores scores in sixteenth-centipawns,
caps corrections at 128 cp, and updates only completed depth-two-or-deeper
non-check nodes whose best move is quiet and whose result is outside the mate
range. Bounds train toward alpha/beta rather than pretending cutoff scores are
exact. No TT, pruning, move-ordering, or SANJ weight changes are bundled.

Require all tests and a deterministic depth-8 tree reduction or no more than a
2% increase. Then run 128 paired games at 20,000 nodes against frozen H3o with
the same N7 10% residual and policy. Reject on a negative point estimate or any
anomaly; only a pass permits a 256-game Blunder 7.6 screen.

## Outcome

All 142 Rust tests and Clippy passed, but H3t expanded the deterministic
depth-8 tree from 536,259 nodes and checksum `9d8d22b14e51010d` to 595,258
nodes and checksum `48d0132b5be406a9`, an 11.00% increase. It failed the
pre-registered 2% ceiling, so no games ran and the playing code was removed.
