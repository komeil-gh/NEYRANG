# REKHNE H19: fused active-stage collection

## Trigger evidence

The fresh H18-PGO depth-12 profile preserved the 29,459,618-node benchmark
and attributed 19.05% of sampled CPU time to
`MovePicker::next_move_with_policy`, still the largest exclusive hotspot.
H18 reduced repeated selection scans, but each good-tactical, killer, and quiet
stage still scans the complete move list once to classify moves and immediately
again to collect the active indices. Current Stockfish likewise scores moves
inside the bounded range that it will subsequently select from rather than
performing a separate whole-list discovery pass.

## Candidate

Reuse H18's existing fixed `u8` active-index array. Reset its length when each
good-tactical, killer, or quiet stage begins, and append a move's original index
at the same moment that classification assigns the stage. Keep the separate
bad-tactical collection pass because losing captures must survive until the
last stage. No new storage, allocation, dependency, score, heuristic, stage
boundary, tie rule, or search decision is allowed.

## Gates

1. Existing exact-order tests plus one focused test must cover fused collection
   across tactical, killer, quiet, bad-tactical, and skipped-quiet paths.
   Formatting, Clippy, and all Linux-applicable repository tests run on SSH.
2. Perft, depth-8 nodes, and checksum must remain exact. Across 31 clean
   interleaved native H18/candidate pairs, paired median throughput must improve
   by at least 1.5%, and neither standalone median may regress.
3. A passing engineering gate opens one clean 1,000-game equal-time parent
   match over 500 reversed opening pairs. Retention requires a non-negative
   point estimate and an independent clean audit.
4. Only a retained source candidate may receive fresh PGO training and a
   same-opening Blunder screen against the frozen H18-PGO deployment baseline.

## Outcome

Rejected. The candidate passed all Linux-applicable repository checks, preserved
Perft 5 (`4,865,609`) and the exact depth-8 tree/checksum
(`536,259`, `9d8d22b14e51010d`), and reduced the 31-pair native median from
250 ms to 243 ms. Paired median throughput improved by 2.88%, clearing the
registered engineering floor.

The independently audited 1,000-game equal-time parent match then scored
`302/382/316` (49.30%, `-4.86 +/-15.29 Elo`) with 1,000 normal terminations
and zero warning, timeout, crash, legality, or protocol anomaly. The negative
point estimate failed gate 3, so the playing change was removed. Per gate 4,
no H19 PGO training or Blunder screen was run.
