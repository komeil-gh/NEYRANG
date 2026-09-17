# N16: symmetric PSQT residual

## Trigger evidence

H34 found frequent large quiet-move errors in real H30 losses. N15 showed that
the existing 128-wide network cannot learn a blended counter-evaluation from
the available corpus. Classical SANJ still supplies 90% of production judgment,
but its piece-square model is only ten compact rank/edge formulae. The opponent
gap is consistent with that capacity limit: Blunder 7.6 reports tuned complete
piece-square tables in addition to tapered material and mobility.

## Frozen candidate

Fit an additive, horizontally symmetric PSQT residual on the unopened N11
training partition only. Use separate middlegame and endgame values for each
piece, relative rank and folded file: 384 integer weights total. Fit the
White-relative teacher minus exact current classical score with sparse ridge
least squares. Evaluate the preregistered ridge values 100, 1,000 and 10,000 on
N11 validation, selecting lowest BCE then MSE, and clamp every rounded residual
to +/-96 centipawns. Do not inspect N11 holdout. N7, EvalMix 10, search and all
non-PSQT evaluation terms remain frozen.

## Gates

1. The feature encoder must prove horizontal symmetry, color/rank symmetry,
   malformed-input rejection and deterministic output in focused tests.
2. The selected rounded vector must improve both validation BCE and MSE over
   current classical SANJ. Then implement only the residual lookup and run the
   complete correctness and symmetry suites.
3. A clean 512-game, 50,000-node parent screen must score at least 50%, followed
   by a clean 512-game equal-time parent screen at or above 50%.
4. Only a passing candidate receives the same-opening 512-game Blunder screen.
   Retention requires exceeding H30's registered 33.79% point estimate without
   a crash, timeout, protocol, legality or opening-pair anomaly.

## Outcome

Rejected and reverted. Ridge 10,000 was selected on the 70,055-position
validation partition. It improved classical BCE from `0.661427306854` to
`0.656889349870` and MSE from `0.041452651816` to `0.039573419704`; all
symmetry, exact 100,000-position trace reconstruction, tactical and perft gates
passed. The required 512-game fixed-node parent match then scored 127 wins,
192 draws and 193 losses: **43.55%**, `-45.04 +/-23.89 Elo`, pentanomial
`[34, 78, 81, 46, 17]`. Every game completed normally, but the result failed
the 50% gate decisively. No equal-time or Blunder match ran; all playing and
trace-schema changes were removed. The experiment is evidence that offline
teacher fit is not a substitute for self-play strength.
