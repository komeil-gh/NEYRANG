# H40: instability-aware soft time budget

## Trigger evidence

H38 and H39 both passed their fixed-node parent screens but failed equal time.
That confirms wall-clock depth, not nominal tree quality alone, is binding at
the registered blitz control. REKHNE currently stops at one fixed soft limit
after every completed iteration, even when the root move changes or the score
drops sharply and the last stable result is least trustworthy.

## Candidate

From completed depth four onward, mark an iteration unstable when its root move
differs from the previous completed iteration or its score falls by at least 30
centipawns. An unstable iteration may continue to 1.5 times the original soft
limit, clamped to the existing hard limit. Stable iterations, hard-deadline
checks, node/depth/infinite limits, search, SANJ, SHEGERD, artifacts, and UCI
options remain unchanged.

## Gates

1. Focused tests must prove stable, extended, and hard-clamped budgets. The full
   quality, timing, tactical, Perft, and UCI suites must pass.
2. Fixed-node depth-8 and depth-12 nodes and checksums must remain exactly H35.
3. A clean 1,000-game paired `0.5+0.005` parent screen against frozen H35 must
   have a non-negative point estimate and zero candidate timing, protocol,
   legality, warning, or opening-pair anomaly.
4. Only a passing parent screen opens the comparable 512-game Blunder 7.6.0
   gate. Retention requires exceeding H35's 33.30% point estimate with no
   candidate anomaly.

## Outcome

Rejected and superseded by a narrower hypothesis.

All correctness gates passed and depth-8/depth-12 remained exactly H35 at
536,259 / 29,459,618 nodes with checksums `9d8d22b14e51010d` /
`0f8416196d8f7941`. Perft 5 remained 4,865,609.

The strict 1,000-game equal-time parent screen scored 298 wins, 390 draws, and
312 losses: 49.30%, `-4.86 +/- 16.61 Elo`. Both engines used one thread, 64 MiB
hash, `0.5+0.005`, and 500 colour-reversed UHO pairs. No candidate timing,
protocol, legality, warning, or opening-pair anomaly occurred. The negative
point estimate failed the gate and no Blunder screen ran. Root-move churn is
too frequent to justify extra time by itself.
