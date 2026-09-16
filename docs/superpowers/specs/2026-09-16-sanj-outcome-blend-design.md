# SANJ H6: teacher and outcome blend

## Hypothesis

N7 learned only the Stockfish WDL expectation stored as its numeric teacher
score. The same audited records also contain the completed game result, but the
trainer fixed its blend coefficient to zero. A result-aware target may teach
positions that lead to wins rather than merely imitating one static score.

Bullet's maintained NNUE examples use a `0.75` WDL proportion. H6 registers
that single value before training; there is no coefficient sweep.

## Candidate

Reuse the frozen 524,288-position N6 corpus and its 20 deterministic exposures,
the N7 Chess768x3hm SCReLU-128 architecture, seeds, optimizer, learning rate,
batch size, feature factorization, and QA1536/QB768/SCALE400 artifact contract.
Change only the training target blend from `0.0` to `0.75`.

The trainer continues to require `position-filter=none` for teacher text because
eligibility was already decided by the audited exporter. Allowing a nonzero WDL
blend does not change which rows are consumed.

## Gates

1. Trainer unit tests and a bounded CUDA smoke fit must pass on the SSH worker.
2. Run exactly one 10,485,760-row fit. The output must be finite, complete, and
   quantize with zero engine/reference mismatches; raw/quantized maximum and
   mean absolute error must remain at most 8 and 2.
3. A clean 512-game, 20,000-node paired screen against embedded N7 must have a
   positive point estimate. Failure removes the candidate.
4. Only a passing parent screen opens a clean 512-game Blunder 7.6.0 screen. It
   must exceed the audited embedded-default reference of 24.02% to be promoted.

No search, ordering, policy, classical evaluation, UCI default, or release asset
changes before those gates pass.

## Outcome

Rejected. The SSH CUDA smoke and the single full fit completed successfully.
The 10,485,760-row run produced a valid 590,628-byte artifact. QA1536/QB768
parity passed on 8,192 registered positions with maximum absolute error
`7.241821289062`, mean absolute error `1.586300603549`, and zero integer
engine/reference mismatches.

The independently audited parent gate then completed 512 normal games with 151
wins, 194 draws, and 167 losses: 48.44%, `-10.86 +/-24.11 Elo`. It used 256
color-reversed pairs, 20,000 nodes, one default engine thread, 64 MiB hash, and
eight concurrent games, with no warning, crash, timeout, protocol, legality, or
replay anomaly.

The negative point estimate failed the registered gate. No Blunder screen was
opened, the trainer restriction was restored, and neither the network nor its
training target is retained.
