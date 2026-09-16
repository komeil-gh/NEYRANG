# SANJ H20: fused classical term traversal

## Trigger evidence

The fresh H18-PGO depth-12 profile preserved the 29,459,618-node benchmark
and attributed 18.25% of sampled CPU time directly to classical evaluation and
another 8.73% to `piece_activity`. The current evaluator walks the same
piece bitboards once for material/PSQT/phase, again for mobility and king
pressure, and a third time over rooks for file bonuses.

## Candidate

Replace those three read-only traversals with one color-local piece pass that
accumulates the existing material, PSQT, phase, mobility, king-pressure, bishop
pair, and rook-file terms. Preserve every coefficient, attack primitive,
integer operation, color sign, phase interpolation, and final side-to-move
score. Do not add caching, storage, dependencies, evaluation features, weights,
or search decisions.

## Gates

1. Formatting, Clippy, all Linux-applicable repository tests, and the existing
   100,000-position SANJ trace reconstruction run on SSH.
2. Perft 5 and both depth-8 and depth-12 nodes/checksums must remain exact.
   Across 31 interleaved native H18/candidate pairs, paired median throughput
   must improve by at least 2.0%, and the candidate standalone median may not
   regress.
3. A passing engineering gate opens one clean 1,000-game equal-time parent
   match over 500 reversed opening pairs. Retention requires a non-negative
   point estimate and an independent clean audit.
4. Only a retained source candidate may receive fresh PGO training and a
   same-opening Blunder screen against the frozen H18-PGO deployment baseline.

## Outcome

Pending. No performance or strength claim exists until every applicable gate
above has completed.
