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

Rejected. The candidate passed every Linux-applicable repository check,
including the 100,000-position SANJ reconstruction. It preserved Perft 5 and
both registered search identities: depth 8 remained `536,259` nodes with
checksum `9d8d22b14e51010d`, while depth 12 remained `29,459,618` nodes with
checksum `0f8416196d8f7941`. Across 31 interleaved native pairs, the median fell
from 254 ms to 245 ms and paired median throughput improved by 3.47%.

The independently audited 1,000-game equal-time parent match scored
`300/386/314` (49.30%, `-4.86 +/-15.83 Elo`) with 1,000 normal terminations
and zero warning, timeout, crash, legality, or protocol anomaly. The negative
point estimate failed gate 3, so the playing change was removed. Per gate 4,
no H20 PGO training or Blunder screen was run.
