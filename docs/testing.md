# Testing and measurement

## Static gates

Every change should pass:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
cargo test --features stats
cargo build --release
```

The suite covers FEN, compact moves, attack edges, legal start moves, hash restoration, randomized make/unmake, three Perft families, evaluation, mate/stalemate/draw search, stop signaling, TT mate normalization, time budgets, SEE, UCI parsing, and an executable UCI smoke test.

Randomized invariant testing uses 64 deterministic seeds and up to 96 legal plies each, then unmakes every move and compares the exact original position.

## Tactical regression suite

Run the selective-search guard independently with:

```bash
cargo test --test tactical
```

The deterministic fixtures cover mate in one, two, and three; a hanging queen; a forced recapture; a quiet mating move; promotion and a promotion race; rook underpromotion to avoid stalemate; a queen-promotion stalemate trap; a KPK opposition position; and the only quiet defensive block. Each case fixes the search depth and checks an exact move or a tablebase-equivalent acceptable move set. The suite also walks every reported PV and verifies that each move is legal in sequence.

## Perft gates

Required release commands:

```bash
target/release/neyrang perft 5
target/release/neyrang perft-fen "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1" 4
target/release/neyrang perft-fen "8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1" 5
```

Expected nodes are 4,865,609; 4,085,603; and 674,624 respectively.

## Deterministic benchmark

The benchmark searches five fixed positions at depth 5. Nodes and checksum must be identical for the same engine revision. Time and NPS are machine- and load-dependent; use `scripts/bench.sh` for repeated runs and compare medians under similar thermal/background conditions.

NEYRANG 0.1.0 ARM64 release baseline on the development Apple Silicon host:

- positions: 5
- depth: 5
- nodes: 448,136
- checksum: `a4b453e8ce750456`
- five-run median: 282 ms and 1,587,507 NPS

All five runs produced the same node count and checksum. This is a deterministic regression baseline, not a cross-machine performance promise.

NEYRANG 0.2.0 on the same workload and host:

- positions: 5
- depth: 5
- nodes: 196,627
- checksum: `4a4c31e290740db3`
- five-run times: 166 / 166 / 166 / 167 / 168 ms
- median time: 166 ms
- median NPS: 1,177,583
- ARM64 release size: 570,656 bytes

The 0.2.0 tree is 56.1% smaller and its local median wall time is 40.7% lower. Median NPS is 26.3% lower, so the improvement comes from search selectivity rather than cheaper nodes. Timing remains machine- and load-dependent.

## Complete-game smoke evidence

On 2026-08-29, the native ARM64 release was run through python-chess 1.11.2 against the official native ARM64 Stockfish 18 binary (SHA-256 `4d77c4aa3ad9bd1ea8111f2ac5a4620fe7ebf998d6893bf828d49ccd579c8cb0`). Five fixed openings were paired with colors reversed for ten games. NEYRANG used depth 3 and Stockfish depth 1.

- 10/10 games completed
- 0 crashes, hangs, illegal moves, or unfinished games
- 5 checkmates and 5 threefold repetitions
- NEYRANG-perspective W/D/L: 4/5/1

The deliberately unequal depth settings make this a protocol/legality stress test only. It is not an Elo estimate or evidence that NEYRANG is stronger than Stockfish.

Two additional paired-color games used `go movetime 20` with a 2 ms move-overhead setting. Both completed after 36 plies without an illegal move, crash, or hang. An interactive infinite-search test returned `bestmove` 0.10 ms after `stop` in the final measured run. These are smoke measurements, not guaranteed latency bounds.

The paired games are retained as an importable PGN at `examples/neyrang-vs-stockfish-smoke.pgn`. The reproducible harness and En Croissant workflow are documented in [the integration guide](en-croissant.md).

## External timing regression

Use the standard-library timing harness to measure UCI deadlines from outside the engine process:

```bash
scripts/timing-audit.py \
  --engine builds/neyrang-v0.2.0 \
  --openings testing/books/8mvs_+90_+99.epd \
  --samples-per-limit 100 \
  --output-json testing/timing-0.3/direct-movetime.json \
  --output-csv testing/timing-0.3/direct-movetime.csv
```

The default schedule runs 100 deterministic, non-repeating positions for each of `go movetime 5`, `10`, `20`, `50`, and `100`, with Hash 64 MB, Threads 1, and seed `20260829`. Pass `--move-overhead 30` for the current `BASE_0_3` default or an explicit historical value when reproducing an older engine. It records the last engine-reported completed `info time`, monotonic wall-clock time through `bestmove`, their gap, and signed overshoot, then reports median, p95, p99, and maximum. The reported direct-harness “latency” is not pure IPC latency: it can include work in an unfinished iteration after the last emitted info line. Use raw samples and strict fastchess PGN latency/time-left telemetry to distinguish search overshoot from scheduler or runner delay.

The [0.3 timing audit](development/0.3.0-timing-audit.md) records the exact architecture, v0.2.0 failures, standalone hardening, final 500 direct samples, and the clean 1,000-game zero-margin stress that defines `BASE_0_3`.

## Strength testing

Run `scripts/test-match-config.sh` before comparative testing. Use `scripts/regression.sh` for paired parent-versus-candidate games, `scripts/match.sh` for fixed-size or node-limited comparisons, and `scripts/sprt.sh` for longer patch decisions. The runners expose a fixed opening seed and record binary/Git/opening/fastchess checksums, complete PGN telemetry, logs, and metadata beside the requested PGN.

Keep engine settings equal, reverse colors, use an audited balanced opening suite, retain PGNs, and report games/W/D/L/score/Elo confidence intervals. The eight bundled openings are only a smoke/development set. The frozen 0.2.0 development configuration is recorded in [the baseline document](development/0.2.0-baseline.md), and feature outcomes belong in [the experiment ledger](development/experiments.md).

## NEYRANG 0.2.0 release evidence

Final versioned release gates:

- `cargo fmt --check`: pass
- Clippy across all targets/features with warnings denied: pass
- normal tests: 47/47 pass; `stats` tests: 52/52 pass
- tactical suite: 11/11 pass, including legal PV replay
- Perft: 119,060,324 / 4,085,603 / 674,624 at the required fixture depths
- fastchess UCI compliance: 40/40 steps pass
- UCI identity/readiness smoke: `NEYRANG 0.2.0`, `uciok`, and `readyok`

The selected SEE/qsearch/LMR candidate was compared with the immutable `v0.1.0` ARM64 binary under a normalized fastchess SPRT:

- opening suite: `8mvs_+90_+99.epd`, SHA-256 `e4ccf9297520743bb44705908b510ae45f817a146418c7773aaf44bed2cf8428`
- seed `20260829`; random order with paired colors
- `0.2+0.002`, Threads 1, Hash 64 MB, concurrency 1
- H0 0 Elo; H1 +5 Elo; alpha/beta 0.05; normalized model
- 786 games / 393 pairs: 389 wins, 192 draws, 205 losses; score 61.70%
- reported Elo +82.87 +/-20.64; nElo +101.38 +/-24.29
- pentanomial `[22, 47, 140, 93, 91]`
- final LLR 2.97 with bounds `[-2.94, +2.94]`: H1 accepted
- no timeout, crash, disconnect, illegal move, or forfeit in the log

The accepted hypothesis says the candidate clears the configured +5 Elo threshold against this exact baseline. It does not say the engine is exactly +5 Elo, nor does the reported estimate transfer automatically to another time control or opponent pool. Raw PGN, log, recovery configuration, and metadata are retained locally under `testing/final/` and summarized in the experiment ledger.

## NEYRANG 0.3 retrospective evidence

Before new 0.3 search work, four fixed 1,000-game matches re-tested every retained 0.2 step with the preserved binaries and the same opening sequence. SEE ordering measured `+29.25 +/-17.09 Elo`, qsearch SEE pruning measured `+66.46 +/-18.72 Elo`, isolated LMR was inconclusive at `+6.25 +/-12.52 Elo`, and cumulative v0.2.0 measured `+90.97 +/-18.38 Elo` against v0.1.0.

All 4,000 games completed with no crash, illegal move, disconnect, or unfinished game. Five 2-11 ms clock forfeits occurred only in historical parent binaries; final LMR/v0.2 recorded none in their 1,000-game appearances. The complete configuration, termination audit, decisions, and artifact checksums are in the [0.3 retrospective campaign](development/0.3.0-game-campaign.md). The separate timing audit subsequently reproduced a current zero-margin edge, hardened it, and passed a fresh 1,000-game stress before the first 0.3 feature experiment.

## NEYRANG 0.3 ordering evidence

The staged MovePicker was kept after 7,046 valid comparison games, including a 4,046-game H1-accepting SPRT and a corrected 1,000-game longer-TC confirmation. The following lazy main-search threshold caller completed two independent 1,000-game screens and a fresh 10,000-game normalized SPRT. The SPRT finished `3557/3487/2956`, 50.35%, `+4.16 +/- 6.81 nElo`, and LLR `+0.69` inside `[-2.94, +2.94]`; it therefore hit the registered inconclusive cap and was reverted rather than promoted.

Independent PGN parsing verified all 10,000 SPRT results, 5,000 paired FENs with exact color swaps, 10,000 `normal` terminations, and no protocol or time failure in the strict log. The complete pre-registration, separate screen results, latency audit, checksums, and revert decision are in the [experiment ledger](development/experiments.md).

The following Capture History candidate completed both mandatory screens before being reverted. At 10,000 nodes per move it scored 363 wins, 355 losses, and 282 draws (50.40%, `+2.78 +/- 7.07 Elo`) over 1,000 games. A fresh equal-time `0.5+0.005` screen scored 352 wins, 351 losses, and 297 draws (50.05%, `+0.35 +/- 13.54 Elo`) over another 1,000 games. Independent parsing verified all 1,000 color-reversed pairs, 2,000 `normal` terminations, exact result reproduction, and no warning or protocol/time failure. These neutral screens did not override the already failed deterministic gate: +4.06% nodes and +3.85% median wall time. Commit `f5450ec` reverted the candidate and passed 62 normal, 67 stats, and 11 tactical tests; the three required Perft values, deterministic benchmark, release build, and fastchess compliance 40/40 also pass.

The 1-ply Continuation History candidate also completed its two 1,000-game diagnostic screens. Its node-limited result was exactly 50.00%, and its recovered equal-time result was 50.80%, but the first equal-time attempt contained a strict candidate timeout. The pre-registered protocol made that failure binding, so commit `3a9e2d9` restored the exact accepted parent before NMP work.

Conservative fixed-R2 NMP then completed 7,946 valid games without a timeout or protocol failure. Its fresh normalized SPRT accepted H1 after 4,946 games at LLR `+2.96` against bounds `[-2.94, +2.94]`; the separate 1,000-game `8+0.08` confirmation scored 54.95%. The feature was retained in frozen source `bb5b8f3`.

Final cumulative validation against immutable v0.2.0 passed the deterministic gate: 8.16% fewer nodes, 11.97% lower median wall time, and 3.91% higher median NPS. The fixed primary screen did not complete cleanly because immutable v0.2.0 twice emitted a PV that continued after threefold repetition. Both histories reproduce directly in the baseline and not in the candidate. The registration allowed only one replacement, so no primary SPRT, holdout match, cumulative longer-TC match, version change, or tag followed.

The final accepted source was rechecked with formatting, Clippy across all targets/features with warnings denied, 63 normal tests, 76 all-feature tests, 11 tactical tests, the three required Perft fixtures, a warning-free release build, and fastchess UCI compliance 40/40. The complete accounting and release decision are in the [0.3 search-development final report](development/0.3.0-final-report.md).
