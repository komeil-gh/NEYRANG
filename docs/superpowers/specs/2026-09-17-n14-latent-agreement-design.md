# N14 latent-agreement SANJ experiment

## Hypothesis

The retained Chess768x3hm network has only a linear output over the two
SCReLU perspective vectors. A third, elementwise product channel may represent
positions where both perspectives activate the same latent concept, without a
second hidden layer or a wider accumulator.

## Frozen candidate

- Artifact/feature-set version: 5 / 5 (`chess768x3hmla`).
- Input transformer: unchanged Chess768x3hm, 128 hidden units.
- Output: `SCReLU(us) || SCReLU(them) || SCReLU(us)*SCReLU(them)`.
- Corpus: registered N11 teacher train and validation partitions.
- Loss: BCE, 200 superbatches, the existing deterministic seed and schedule.
- Quantization: QA=1536 and the existing output quantization contract.
- Final holdout remains sealed until one checkpoint and mix are frozen.

## Gates

1. Scalar engine, float oracle, quantized oracle, artifact round-trip, and
   accumulator-update parity must pass.
2. The selected checkpoint must improve both BCE and blended-score MSE on the
   registered validation partition versus retained SANJ-N7 at EvalMix=10.
3. Only a candidate passing gate 2 may enter exact fixed-node A/B, then
   equal-time A/B, then equal-resource Blunder testing.
4. Any correctness mismatch, offline regression, or game regression rejects
   the candidate and restores the retained H30 source and SANJ-N7 asset.

## Outcome

Rejected before holdout and game testing. The registered CUDA run completed all
200 superbatches and produced twenty QA-1536 checkpoints. The best checkpoint
by validation BCE was superbatch 20. Even after independently scanning all
integer EvalMix values, it regressed both registered metrics versus retained
SANJ-N7 at EvalMix 10:

- BCE: 0.660206089442 versus 0.654985941053.
- MSE: 0.040854280626 versus 0.038524874335.

The implementation passed artifact round-trip, float/quantized parity, and
bit-exact engine/reference checks before training. Since the offline strength
gate failed, the holdout remained sealed and the experimental version-5 code
was removed from the playing and tooling sources. SANJ-N7 remains retained.
