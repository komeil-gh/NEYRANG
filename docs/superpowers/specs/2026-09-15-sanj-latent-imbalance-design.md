# SANJ H5h: latent dual-perspective imbalance

## Hypothesis

H5f and H5g show that material-phase calibration is not the missing capacity:
one reduced fresh teacher error without a conclusive interval and the other
regressed. The retained linear head can value each hidden neuron from either
perspective, but it cannot directly value the magnitude of a disagreement
between the two activated perspectives. Such disagreements can encode an
asymmetric latent motif even when neither activation alone carries a stable
linear meaning.

## Candidate

`Chess768x3hmli` keeps the existing factorised three-bank incremental feature
transformer and SCReLU-128 activations. Its only new input is the elementwise
absolute difference `abs(SCReLU(us) - SCReLU(them))`. One scalar affine head
therefore consumes 384 values instead of 256. This adds 128 output weights,
128 subtractions, 128 absolute values, and 128 multiply-accumulates per static
evaluation; it adds no hidden layer, feature refresh, phase branch, dependency,
search heuristic, corpus, or UCI option.

Artifact version 4 and feature-set id 4 carry the 384 output weights. Versions
1 through 3 remain byte-compatible. The independent float evaluator, quantized
reference, and engine runtime must use the same post-SCReLU difference.

## Gates

1. Format round-trip, integer engine/reference parity, float/quantized parity,
   trainer tests, formatting, Clippy, and the full quality gate must pass.
2. Train exactly one candidate from the approved N6 corpus with the frozen N7
   schedule and seeds. A small CUDA smoke test must pass before the full fit.
3. A new deterministic validation set must be unique and disjoint from H5f,
   H5g, the approved corpus, and all N6-N9 selected roots.
4. Quantized Stockfish-18 WDL-space MSE must beat N7 with a strictly negative
   paired-bootstrap 95% upper bound. Failure ends H5h before engine games.
5. A passing candidate faces H4m in independent fixed-node and equal-time
   parent screens. Only both passes open a fresh Blunder 7.6.0 screen.

## Result

Pending.
