# SHEGERD H17: stage-local policy phase reuse

## Trigger evidence

The H15-PGO profile placed 29.55% of samples in MovePicker. The retained
SHEGERD policy recomputes the same material-phase bucket for every scored move,
even though the position does not change while one MovePicker stage is scored.

## Candidate

Compute the policy phase once when a tactical, killer, or quiet stage is first
scored and reuse that exact bucket for every move in the stage. Keep the public
policy-score path as the reference oracle. No table, heuristic, weight, stage
boundary, score, or search decision may change.

## Gates

1. A deterministic legal-play test must prove cached-phase policy scores equal
   the existing reference score for every sampled move. All repository tests,
   formatting, and Clippy must pass on SSH.
2. Perft, depth-8 nodes, and checksum must remain exact. Across 31 interleaved
   native pairs, paired median throughput must improve by at least 1.5% over
   frozen H14 and neither standalone median may regress.
3. A passing engineering gate opens one clean 1,000-game equal-time parent
   match on 500 reversed opening pairs. Retention requires a non-negative point
   estimate and an independent clean audit.
4. Only a retained candidate may be rebuilt with a fresh PGO profile or receive
   an external Blunder screen.

## Outcome

Rejected at the engineering gate on 2026-09-16. The focused legal-play test
passed, and both candidates produced the exact 536,259-node depth-8 tree and
checksum `9d8d22b14e51010d`. Across 31 interleaved native pairs, however, the
frozen H14 median was 256 ms, the candidate median was 259 ms, and paired
median throughput changed by -1.53%. This missed the registered +1.5% floor
and also violated the no-regression condition. Zero games ran, the playing
code was removed, and no strength claim exists.
