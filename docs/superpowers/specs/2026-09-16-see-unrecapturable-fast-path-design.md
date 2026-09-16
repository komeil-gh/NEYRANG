# SHEGERD H12: unrecapturable SEE fast path

## Trigger evidence

The retained H9 stats benchmark performs 628,542 exact SEE calls at depth eight;
350,513 scored tactical moves are never searched. MovePicker needs exact SEE to
preserve its current class and order, so H12 does not replace it with MVV-LVA
or a threshold approximation. Instead it targets the cheapest exact case: a
tactical move whose destination has no geometric enemy recapture after the
move.

## Candidate

Reuse the legal move generator's occupancy-aware attacker query. Before copying
the twelve piece bitboards and entering the recursive exchange, construct only
the final occupancy and removed-enemy mask. If no enemy piece attacks the
destination, return the exact captured-plus-promotion gain. Otherwise run the
unchanged SEE implementation.

Apply the same exact shortcut to `see_ge`. Move scores, classes, policy buckets,
search, SANJ/NNUE, pruning, artifacts, and UCI defaults remain unchanged.

## Gates

1. A focused regression covers the shortcut, and the 100,000-position immutable
   SEE oracle must preserve every exact score and threshold result. Formatting,
   Clippy, all Rust tests, the tactical suite, perft 5, and SANJ trace oracle
   must pass on SSH.
2. The host-native depth-8 tree, best moves, scores, and checksum must be exact.
   Across 21 interleaved pairs, retain H12 only for at least 2% lower median
   wall time than frozen H9.
3. A passing engineering gate opens 1,000 clean paired `0.5+0.005` games against
   H9. A positive point estimate is required before a same-opening 1,000-game
   Blunder screen.
4. External progress requires a score above H9's retained 28.10%. Confidence
   intervals remain mandatory; crossing zero is reported as directional only.

## Outcome

Retained. The 100,000-position immutable SEE oracle, SANJ trace oracle, full
all-feature test suite, Clippy, formatting, tactical suite, and perft gate all
passed on the SSH worker. H12 preserved the exact 536,259-node depth-8 tree and
`9d8d22b14e51010d` checksum. Across 21 interleaved host-native pairs its median
fell from 330 ms to 313 ms, a 5.43% throughput improvement.

The independently audited 1,000-game parent match scored 50.45% against H9
(`325/359/316`, `+3.13 +/-16.76 Elo`) with 1,000 normal terminations and no
warning, timeout, crash, legality, or protocol anomaly. The same-opening
1,000-game Blunder screen scored 29.75% (`163/269/568`) versus H9's retained
28.10%. The paired difference is +1.65 percentage points with a 20,000-sample
paired bootstrap 95% interval of -1.70 to +5.10 points. H12 therefore meets the
pre-registered external point-estimate floor, but its external gain is
directional rather than statistically proven.
