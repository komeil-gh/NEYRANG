# N15: SANJ counter-evaluator

## Trigger evidence

The H30 versus Blunder loss corpus contains 7,557 analysed NEYRANG decisions
while Stockfish still scores the position above -800 centipawns. At 20,000
teacher nodes per candidate, 444 decisions lose at least 150 centipawns. Quiet
moves account for 337 of those errors. H30 already searches more nodes and often
one more nominal ply than Blunder at the registered ultra-short control, so the
next candidate targets leaf judgment rather than another isolated pruning rule.

## Frozen candidate

Reuse the unopened N11 train and validation partitions, Chess768x3hm SCReLU-128
architecture, deterministic seeds, optimizer, QA1536/QB768/SCALE400 contract,
and 200-superbatch MSE schedule. Do not read the N11 holdout.

For a frozen production blend of 75%, transform each teacher score into the
network-side score which reconstructs that teacher after blending with the
current classical SANJ score. Clamp only to the existing +/-3040 input contract.
This trains the network to counter the measured classical error instead of
learning a second independent full evaluation. No runtime format or search rule
changes are allowed.

## Gates

1. The converter must preserve row/FEN alignment, side-to-move orientation and
   determinism; its focused tests and all trainer tests must pass.
2. Train one lane only. Quantize every ten-superbatch checkpoint at QA1536 and
   select the checkpoint with the lowest validation BCE, breaking ties by MSE.
3. At EvalMix 75, the selected candidate must improve both validation BCE and
   MSE over retained N7 at EvalMix 10. Only then open the untouched N11 holdout
   once and require both metrics to improve there as well.
4. A passing artifact must pass raw/quantized/reference/runtime parity, the full
   correctness suite, 512 fixed-node parent games at or above 50%, and 512
   equal-time parent games at or above 50% with no anomaly.
5. Only a passing parent candidate receives a 512-game equal-resource Blunder
   screen. Retention requires exceeding H30's registered 33.79% point estimate.

## Outcome

Rejected before holdout or game testing. The deterministic converter produced
540,672 training rows and 70,055 validation rows; 52,415 and 6,663 targets,
respectively, reached the existing score clamp. The single 200-superbatch CUDA
lane completed and all twenty QA1536 checkpoints were evaluated at the frozen
75% blend. Checkpoint 10 was best, but it regressed both registered validation
metrics versus N7 at EvalMix 10: BCE `0.669282142557` versus
`0.654985941053`, and MSE `0.045091363172` versus `0.038524874335`.
The untouched holdout remained sealed and no runtime or game gate was opened.
