# H32: fresh native PGO for retained H30

## Trigger evidence

H30 preserves the exact search tree while improving native median throughput by
29.87%, but its accepted source has never received a fresh profile-guided build.
Earlier retained source gained another 3.29% over its native binary from PGO.
The audited H30 Blunder score remains only 33.79%, and deeper completed searches
have previously shown lower Stockfish teacher loss.

## Candidate

Build one host-native PGO artifact from the retained H30 source. Train the
instrumented binary on 256 paired 50,000-node self-play games using the frozen
UHO suite, one engine thread, 64 MiB hash, and eight concurrent games. PGO may
change machine code only; source, search decisions, N7 `EvalMix=10`, P3 policy,
UCI defaults, and artifacts remain frozen.

## Gate

1. The training match must complete normally and produce a mergeable profile.
2. The optimized artifact must preserve Perft, depth-8 and depth-12 node counts
   and checksums, UCI behavior, and the focused scalar/AVX2 oracle.
3. Across 31 interleaved pairs on the idle worker, PGO median throughput must be
   at least 1.5% above the H30 native binary without a slower median.
4. A passing artifact receives one fresh 512-game equal-resource Blunder 7.6.0
   screen. It must exceed H30's audited 33.79% point estimate and have no
   candidate anomaly to be retained as the host deployment binary. Results do
   not transfer to other CPUs.

## Outcome

Rejected at the throughput gate. The 256-game profile workload completed with
66 wins, 66 losses, 124 draws, pentanomial `[0, 0, 128, 0, 0]`, and no audit
anomaly. The optimized binary preserved Perft 5 (`4,865,609`), the depth-8
result (`536,259`, checksum `9d8d22b14e51010d`), the depth-12 result
(`29,459,618`, checksum `0f8416196d8f7941`), and the focused wide-AVX2 oracle.

Across 31 idle-worker interleaved pairs, H30 native measured a median
`2,398,156` NPS while H32 PGO measured `2,336,039` NPS: **-2.59%**. H32 won
only 5 of 31 paired measurements. Because it missed the predeclared +1.5%
gate and was slower, no Blunder screen was run and H30 remains the retained
binary.
