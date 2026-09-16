# REKHNE H18: compact active-stage range

## Trigger evidence

The fresh H15-PGO profile placed 29.55% of samples in
`MovePicker::next_move_with_policy`. The current `pick_best` scans every legal
move on each delivery, including moves belonging to other stages and moves
already returned. Current Stockfish instead keeps a bounded current range and
selects only inside that range.

## Candidate

Keep the existing fixed-capacity `MovePicker`, scoring, stage boundaries, and
lazy stage initialization. When a stage opens, collect only that stage's
original move indices into one fixed `u8` array. `pick_best` then scans the
active prefix, resolves equal scores by the original index, and removes the
winner with `swap_remove`. This must reproduce the current score-descending,
original-order tie behavior while avoiding repeated full-list scans. No heap
allocation, new dependency, heuristic, weight, or search decision is allowed.

## Gates

1. Focused tests must prove exact delivery order against the existing behavior,
   including equal-score ties, preferred/killer suppression, tactical stages,
   and early quiet skipping. Formatting, Clippy, and all repository tests run
   on SSH.
2. Perft, depth-8 nodes, and checksum must remain exact. Across 31 interleaved
   native H14/candidate pairs, paired median throughput must improve by at
   least 1.5%, and neither standalone median may regress.
3. A passing engineering gate opens one clean 1,000-game equal-time parent
   match over 500 reversed opening pairs. Retention requires a non-negative
   point estimate and an independent clean audit.
4. Only a retained source candidate may receive fresh PGO training and a
   same-opening Blunder screen against the frozen H15 deployment baseline.

## Outcome

Pending. No result or strength claim exists until every applicable gate above
has completed.
