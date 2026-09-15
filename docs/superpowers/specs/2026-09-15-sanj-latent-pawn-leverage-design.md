# SANJ Latent Pawn Leverage

## Hypothesis

SANJ counts pawn structure but does not value a pawn that attacks a more
valuable piece or can make a safe one-square push that creates such an attack.
A small victim-sensitive leverage term can prefer forcing, positionally sound
pawn play without teaching REKHNE a style override.

## Boundary and gate

H3w adds only a symmetric tapered SANJ term. Direct pawn attacks and quiet
one-square pawn pushes score by the attacked non-pawn piece; a latent push is
ignored when its destination is occupied, is a promotion, or is attacked by an
enemy pawn. Each side's contribution is capped at 80 centipawns. Search,
ordering, NNUE, policy, and every pruning margin remain unchanged.

Require all Rust tests, Clippy, and the tactical suite. Then run 128 fresh
paired openings at 20,000 nodes against frozen H3o with the same N7 10%
residual and policy. Reject on a negative point estimate or any anomaly. Only
a pass permits a separate fresh 256-game Blunder 7.6.0 screen.

## Local gate

All 142 Rust tests, the 11-test tactical suite, and Clippy passed. With the
frozen N7 network, stage-aligned policy, and `EvalMix=10`, the five-position
depth-8 tree changed from 513,582 to 519,454 nodes (+1.14%). Four best moves
were unchanged; the equal-scored start-position choice changed from `d2d4` to
`g1f3`. The parent screen is open.

## Outcome

The clean 256-game parent screen scored 80 wins, 73 losses, and 103 draws
(51.37%, `+9.50 +/-29.79 Elo`). The separately selected Blunder screen then
scored 35/167/54 (24.22%, `-198.16 +/-42.62 Elo`), below the retained H3o
hybrid's earlier 26.56% point estimate. All 256 games and 128 pairs completed
with empty stderr and zero exit status. H3w is rejected and its playing code
and trace extension were removed.
