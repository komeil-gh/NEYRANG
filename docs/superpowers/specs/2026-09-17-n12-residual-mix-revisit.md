# N12: independently selected N7 residual mix

## Trigger evidence

The retained N7 network is stronger than every fresh N10/N11 teacher-trained
candidate on their validation partitions, but the release still uses a 10%
NNUE residual selected by an earlier small game screen.  On two disjoint
Stockfish-18 validation corpora, the existing N7/classical blend minimized BCE
at 64% (302,804 positions, 2,000-node labels) and 26% (70,055 positions,
20,000-node labels).  The previous 15% game screen was only a 128-game point
estimate and did not test the independently indicated range.

## Candidate

Freeze the current H30 binary, embedded N7 artifact, embedded P3 policy, search
rules, time control, and openings.  Test exactly one conservative value,
`EvalMix=25`, against the retained `EvalMix=10`.  No other mix may be selected
from the game results.

## Gate

1. A 512-game paired 50,000-node parent screen must have no candidate anomaly
   and a non-negative point estimate.
2. If it passes, a fresh 512-game paired `0.5+0.005` screen must also have no
   candidate anomaly and a non-negative point estimate.
3. Only if both parent gates pass may the frozen 25% candidate run a fresh
   equal-resource Blunder 7.6.0 screen.  Promotion requires a higher score than
   H30's audited 33.79% point estimate; confidence intervals remain explicit.

## Outcome

Rejected on the first gate. The complete 512-game fixed-node match scored
106 wins, 222 losses, and 184 draws for the 25% candidate: 38.67%,
-80.11 +/- 23.81 Elo, and pentanomial `[47, 75, 93, 29, 12]`. The independent
audit found all 512 games normal, all 256 opening pairs present, balanced
colors, matching engine hashes, and no warning, crash, timeout, protocol, or
legality anomaly. Because the point estimate was negative, the registered
equal-time and Blunder gates were not run. `EvalMix=10` remains retained.
