# SANJ H5g: shared judgment with material-phase calibration

## Hypothesis

H5f's four independent output projections reduced fresh teacher MSE by 2.03%,
but the improvement was not statistically established. Splitting every output
weight also asks each material bucket to learn its own judgment from only part
of the corpus. H5g keeps one shared 256-to-1 projection and learns only four
material-routed scalar biases. This preserves cross-phase evidence while adding
three effective calibration degrees of freedom.

## Candidate

The trainer-only `chess768x3hm4pb` graph uses the existing Chess768x3hm input
transformer, SCReLU-128 dual perspective, one shared output vector, and four
zero-initialised phase biases selected by the existing `(piece_count - 2) / 8`
rule. Export repeats the shared vector into the existing version-3 four-head
artifact, so engine inference, the decoder, the accumulator, and the artifact
contract do not change.

No new corpus, dependency, search heuristic, UCI option, runtime branch, or
artifact version is introduced.

## Gates

1. Trainer tests, formatting, and Clippy must pass.
2. Train one candidate from the approved N6 corpus with the frozen N7 schedule
   and seed.
3. On a new root-disjoint validation set, quantised MSE must beat N7 with a
   strictly negative paired-bootstrap 95% upper bound. Failure ends H5g before
   games.
4. A passing candidate must pass float/quantised parity before independent
   fixed-node and equal-time parent screens. Both point estimates must be
   non-negative with zero anomalies.
5. Only both parent passes open a fresh Blunder screen; H4m remains untouched
   until all gates pass.

## Result

H5g is rejected before games. The full fit consumed the frozen 10,485,760-row
schedule and exported a valid version-3 artifact. On 2,048 new deterministic
positions, Stockfish-18 WDL-space MSE increased from `0.09202358` to
`0.09310036` (`-1.17%` relative improvement). The paired mean delta was
`+0.00107678` with a deterministic 10,000-replicate bootstrap 95% interval of
`[-0.00094977, +0.00315776]`. The positions were unique and had zero overlap
with H5f, the approved N6 corpus, or the complete N6-N9 selected-root chain.
No H5g engine game was opened and its network is not retained.
