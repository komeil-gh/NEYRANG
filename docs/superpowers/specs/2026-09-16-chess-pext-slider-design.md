# CHESS H14: native PEXT slider attacks

## Trigger evidence

The retained H12 binary spent most sampled CPU time in paths that repeatedly
request sliding attacks: SANJ evaluation, legal move generation, attacker
queries, and SEE. A 16.6-second depth-12 SSH profile attributed 21.1% directly
to classical evaluation, 19.9% to legal move generation, 11.2% to SEE
recaptures, 10.6% to piece activity, 8.7% to `attackers_to`, and 6.8% to
slider generation. These shares overlap through the scalar ray backend.

## Candidate

Keep the existing directional-ray implementation as the portable reference and
fallback. On x86-64 builds compiled with BMI2, initialize compact bishop and
rook attack tables once and index them with `PEXT`. Queen attacks remain the
union of bishop and rook attacks. Search, evaluation, move generation, and all
scores stay unchanged.

The tables contain only relevant inner-ray occupancy bits: 5,248 bishop entries
and 102,400 rook entries, or 841 KiB of attack data. No dependency, runtime CPU
dispatch, or new UCI option is added; the optimized path is available only to
the already-retained `target-cpu=native` deployment build.

## Gates

1. The existing independent coordinate-stepping oracle must check every square
   and all 1,119,744 full-ray occupancy subsets under native BMI2.
2. Formatting, Clippy with warnings denied, the full Rust suite, perft,
   tactical tests, the 100,000-position move/SEE/SANJ oracles, and the
   OpenBench contract must pass on SSH.
3. The host-native depth-8 tree and checksum must remain exactly 536,259 nodes
   and `9d8d22b14e51010d`.
4. Across 21 interleaved depth-8 pairs, median wall time must improve by at
   least 5% over frozen H12. A passing engineering gate opens a 1,000-game
   equal-time parent match; retention requires a non-negative point estimate
   and a clean independent audit.

## Outcome

Accepted as H14.

- The independent 1,119,744-subset slider oracle, formatting, Clippy with
  warnings denied, all-feature Rust suite, 100,000-position move/SEE/SANJ
  oracles, 11-position tactical suite, perft 5, and OpenBench contract passed
  on the SSH worker.
- H14 preserved the exact 536,259-node depth-8 tree and
  `9d8d22b14e51010d` checksum. Across 21 interleaved pairs, the H12 median was
  289 ms and H14 was 257 ms, a 12.45% throughput improvement.
- Its strict 1,000-game equal-time parent match scored `334/374/292` (52.10%,
  `+14.60 +/-16.02 Elo`). Independent replay found 1,000 normal terminations
  and no warning, timeout, crash, legality, or protocol anomaly.
- The same-opening Blunder 7.6.0 screen scored `160/280/560` (30.00%,
  `-147.19 +/-18.97 Elo` relative to Blunder), versus H12's 29.75%. The
  +0.25-point external difference is directional only and not statistically
  proven. All 1,000 games terminated normally; three opponent-only draw-rule
  PV warnings matched the registered compatibility policy.
