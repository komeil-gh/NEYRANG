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

Retained on 2026-09-16. Focused ordering tests, formatting, Clippy, all
Linux-applicable Rust suites, 159 Python infrastructure tests, the match
contract, and the OpenBench contract passed on the SSH worker. The candidate
preserved the exact 536,259-node depth-8 tree and checksum
`9d8d22b14e51010d`. One timing attempt was excluded after a contemporaneous
process listing established an unrelated six-worker CPU workload. In the clean
registered 31-pair rerun, H14's median was 265 ms, H18's was 254 ms, and paired
median throughput improved by 4.49%.

The independently audited 1,000-game equal-time parent match scored
`311/402/287` for 51.20% (`+8.34 +/-15.29 Elo`) with pentanomial
`[30,114,194,126,36]`, 1,000 normal terminations, complete telemetry, and no
timing, legality, crash, warning, or protocol anomaly. This met the registered
non-negative retention floor.

A fresh 256-game fixed-node profile produced the retained host-native H18-PGO
artifact. It preserved the same tree/checksum and improved paired median
throughput by 3.29% over native H18 and 2.97% over H15-PGO. In the clean
same-opening Blunder screen, H18-PGO scored `188/305/507` (34.05%) versus the
frozen H15-PGO result of 31.85%. The 20,000-sample paired difference was +2.20
percentage points with a 95% interval of -1.15 to +5.50, or +17.30 logistic
Elo with a paired interval of -9.17 to +43.24. The external gain is directional,
not statistically proven.
