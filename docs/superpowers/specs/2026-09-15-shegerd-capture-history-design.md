# SHEGERD Capture History

## Hypothesis

Exact SEE separates sound captures from losing ones, but equal-stage captures
are still ordered mostly by fixed material rules. Remembering which
attacker-target-victim captures caused or failed a beta cutoff can make the
existing staged MovePicker and LMR spend nodes on empirically useful tactics.

## Boundary and gate

H3y adds one search-local capture-history table indexed by color, attacker,
destination, and victim. Only searched captures update it: the cutoff capture
is rewarded and earlier failed captures are penalized with the existing
bounded history update. The value orders captures inside their existing good
or bad SEE stage; it cannot move a losing capture ahead of quiets. SANJ,
policy, SEE classification, pruning, reductions, and UCI defaults are unchanged.

Require all Rust tests, Clippy, the tactical suite, and no more than 2% growth
in the frozen depth-8 hybrid tree. Then run 128 fresh paired openings at 20,000
nodes against H3o. Reject on a negative point estimate or anomaly. Only a pass
opens a fresh 256-game Blunder 7.6.0 screen, which must exceed the retained
26.56% point estimate.

## Outcome

All 142 Rust tests, the 11-test tactical suite, and Clippy passed, but H3y
expanded the five-position depth-8 tree from 513,582 to 555,300 nodes (+8.12%)
while preserving all best moves and scores. It failed the registered 2% ceiling,
so no games ran and the playing code was removed.
