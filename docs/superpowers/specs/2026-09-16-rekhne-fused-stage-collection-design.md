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

Pending. No performance or strength claim exists until every applicable gate
above has completed.
