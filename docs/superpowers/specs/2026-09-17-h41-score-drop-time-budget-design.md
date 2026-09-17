# H41: score-drop-only soft time budget

## Trigger evidence

H40's combined root-move-change/score-drop trigger lost its 1,000-game parent
screen at 49.30% without any timing anomaly. Root-move churn alone therefore
caused unproductive overspending. A fall of at least 30 centipawns between
completed iterations is the remaining direct signal that the last stable
result became less trustworthy.

## Candidate

From completed depth four onward, allow only an iteration whose score fell by
at least 30 centipawns to continue to 1.3 times the original soft limit,
clamped to the existing hard limit. Root-move changes alone do nothing. Search,
hard deadlines, node/depth/infinite limits, SANJ, SHEGERD, artifacts, and UCI
options remain frozen on H35.

## Gates

1. Focused stable, extended, and hard-clamp tests plus the full quality,
   timing, tactical, Perft, and UCI suites must pass.
2. Depth-8 and depth-12 nodes/checksums must remain exactly H35.
3. A clean 1,000-game paired `0.5+0.005` parent screen must have a non-negative
   point estimate and no candidate anomaly.
4. Only a passing parent opens the comparable 512-game Blunder 7.6.0 screen.
   Retention requires exceeding H35's 33.30% point estimate without a candidate
   anomaly.

## Outcome

Accepted as a bounded time-management improvement.

The full quality and timing suites passed. Depth-8 and depth-12 stayed exactly
at 536,259 / 29,459,618 nodes with checksums `9d8d22b14e51010d` /
`0f8416196d8f7941`; Perft 5 stayed at 4,865,609.

The strict 1,000-game equal-time parent screen scored 315 wins, 380 draws, and
305 losses: 50.50%, `+3.47 +/- 15.20 Elo`. Both engines used one thread, 64 MiB
hash, `0.5+0.005`, and 500 colour-reversed UHO pairs. There was no candidate
timing, protocol, legality, warning, or opening-pair anomaly.

On the exact 256 opening pairs used by the registered H35 comparison, H41
scored 111 wins, 144 draws, and 257 losses against Blunder 7.6.0: 35.74%,
`-101.90 +/- 26.47 Elo`. H35 had scored 33.30% on the same control and pairs,
so the point estimate improved by 2.44 percentage points. One allowed immutable
Blunder post-threefold PV warning occurred; no candidate anomaly occurred and
the compatibility audit passed. The external difference is directional, not a
statistically proven Elo gain, and H41 still loses decisively to Blunder.
