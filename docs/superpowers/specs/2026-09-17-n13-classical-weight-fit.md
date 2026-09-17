# N13: bounded SANJ classical-weight fit

## Trigger evidence

The retained SANJ classical evaluator still uses 42 hand-tuned effective
weights. Previous search and network experiments cannot correct systematic
error in that component, and N11 left a large, game-partitioned Stockfish-18
teacher corpus whose holdout remains sealed.

## Candidate

Convert only N11 train and validation teacher rows to canonical `sanj-trace`
features. Preserve conservative whole-sequence groups, bind the exact feature
hashes and current source vector before fitting, and run one deterministic
bounded L-BFGS-B fit. Keep material ordering, king-danger behavior, N7,
`EvalMix=10`, search, and UCI defaults frozen. Do not read the holdout.

## Gates

1. The trace must exactly reconstruct the current evaluator on every row, with
   no train/validation group overlap.
2. The rounded vector must stay inside its registered bounds, preserve material
   ordering, and improve both grouped cross-entropy and MSE on train and
   validation. Both validation 95% group-bootstrap upper endpoints must be
   below zero. Two complete fits must be byte-identical.
3. A passing vector is applied exactly once, then must preserve Perft, UCI, and
   NNUE/runtime parity and score at least 50% in a clean 256-game paired
   50,000-node parent screen.
4. Only a passing fixed-node candidate receives a clean 512-game paired
   equal-time parent screen. It must score at least 50% before any fresh
   equal-resource Blunder gate. Offline loss alone is not strength evidence.

## Outcome

Rejected at the first game gate. The registered fit used 540,672 train and
70,055 validation positions in 12,134 and 1,556 conservative sequence groups.
The frozen vector improved both grouped losses on both partitions, both
validation bootstrap upper endpoints were below zero, and two complete fits
were byte-identical (`106501fe73ed98bdb7dd48b35e8a3fb1e87cd5fd5813a4cc7cdbd80c5a9ada39`).
The holdout remained unopened.

The exact rounded vector then scored 72 wins, 82 losses, and 102 draws against
H30 in the preregistered 256-game 50,000-node parent screen: **48.05%**,
`-13.58 +/- 32.20 Elo`, pentanomial `[12, 31, 51, 23, 11]`. The independent
audit found 256 normal terminations and no warning, crash, timeout, protocol,
or opening-pair anomaly. Because 48.05% is below the 50% gate, the vector was
reverted; no equal-time or Blunder screen was run and the original SANJ weights
remain retained.
