# REKHNE H15: profile-guided native deployment

## Trigger evidence

After H14 enabled native BMI2 slider tables, a fresh depth-12 profile still
placed most samples in branch-heavy search work: classical SANJ, staged move
selection, SEE, legal move generation, attacker queries, and make/unmake. These
paths were already semantically guarded, so the next candidate targeted code
layout and branch placement without another search heuristic.

## Candidate

Keep the complete H14 source, search decisions, embedded N7 residual, policy,
and UCI defaults frozen. Build a separate `target-cpu=native` artifact with
Rust profile-guided optimization. Collect the profile from 256 paired
self-play games at 50,000 nodes per engine through the normal UCI path, then
merge the raw profiles and rebuild the same source with `profile-use`.

This is compiler feedback, not chess learning. The portable release remains
unchanged, and the resulting binary is specific to the training workload,
compiler, and CPU family.

## Gates

1. The instrumented training run must complete 256 normal games with no
   protocol or game anomaly and exercise the default embedded evaluator and
   policy.
2. Perft 5 and the native depth-8 tree/checksum must remain exact.
3. Across 31 interleaved depth-8 pairs, paired median throughput must improve
   by at least 3% over frozen H14.
4. A clean 1,000-game equal-time parent match must have a positive point
   estimate. A pass opens a same-opening 1,000-game Blunder 7.6.0 screen; that
   external result is reported with a paired bootstrap interval rather than as
   a release-rating claim.

## Outcome

Accepted as the host-specific H15 deployment artifact.

- Training completed 256 normal games at 50,000 nodes per engine, with
  `112/32/112` and no anomaly.
- Perft 5 remained 4,865,609. Depth 8 remained 536,259 nodes with checksum
  `9d8d22b14e51010d`.
- Across 31 interleaved pairs, H14's median was 259 ms and H15's was 251 ms;
  paired median throughput improved by 3.23%.
- H15 scored `342/359/299` against H14 in 1,000 clean equal-time games:
  52.15%, or `+14.95 +/-16.63 Elo`.
- Against Blunder, H15 scored `184/269/547` (31.85%) versus H14's 30.00% on
  the identical 500 opening/color pairs. The paired gain is +1.85 percentage
  points (95% interval -1.20 to +4.95), or +15.05 logistic Elo (paired 95%
  interval -9.82 to +40.49). The external gain is directional, not
  statistically proven.

The retained H15 binary SHA-256 is
`68641f36e2b993258e06be5fc87c75ac44b16266f8674ceb015ab9e5667ed1c5`.
