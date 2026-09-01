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

## OpenBench repository contract

OpenBench public workers build an engine through `make EXE=<assigned-name>` and
require the executable beside that Makefile. Their bench parser runs
`./<binary> bench`, extracts nodes/NPS, repeats the command concurrently, and
rejects missing or non-deterministic results.

NEYRANG's root Makefile implements that build contract. Verify it independently of
the normal Cargo build with:

```bash
scripts/test-openbench-contract.sh
```

The test creates a unique temporary executable, requires three sequential and
three concurrent 180,591-node benches with positive NPS, verifies UCI `Hash`,
`Threads`, `uciok`, and `readyok`, repeats an eight-opening `genfens` workload,
checks its exact line shape and upper-seed-bit sensitivity, and removes every
temporary artifact on each exit path. This is a build and protocol gate, not
game-strength evidence.
Deployment prerequisites and the server-side configuration template are in
[the OpenBench guide](openbench.md).

## Deterministic opening-generation gates

`tests/genfens.rs` fixes the engine-facing contract: legal nonterminal output,
no check on the side to move, deterministic diversity, full 64-bit seed use,
batch/shard equivalence, strict rejection, and the exact two-argument OpenBench
process shape. The Python infrastructure suite separately checks atomic shard
publication, provenance, canonical duplicate rejection, malformed FEN rejection,
overwrite refusal, and process termination after a simulated stall.

Run both layers with:

```bash
cargo test --test genfens --locked
.venv/bin/python -m unittest scripts.tests.test_generate_opening_shard
```

For a retained shard, use `scripts/generate-opening-shard.py`; do not redirect
raw engine stdout into a file and call it audited data. Generated EPD/manifests
belong under the ignored `testing/` tree or external content-addressed storage,
not Git. This gate certifies openings and provenance only, not self-play labels,
NNUE scale, offline loss, or Elo.

## Lazy-SMP correctness and scaling

The P3 candidate keeps the direct single-thread semantic gates at 180,591 nodes/checksum `e04f83f9a9880115` for depth 5 and 4,483,694/checksum `d8d4a9b6bbcde023` for depth 8. Targeted tests cover packed shared-TT fields, signatures, replacement, generation, clearing, memory budget, coherent concurrent publication, root voting, exact aggregate node limits, terminal roots, depth searches, pre-signalled/external stop, and UCI depth/node/movetime/infinite behavior with exactly one bestmove.

Run the pre-registered external scaling gate only on a release build with no concurrent CPU-heavy workload:

```bash
.venv/bin/python scripts/smp-bench.py \
  --engine target/release/neyrang \
  --output testing/smp/p3-scaling.json
```

The fixed command performs one excluded warm-up for each of Threads 1/2/4 and seven measured rotating-order runs. Every run searches the same five benchmark FENs at `go movetime 1000`, total Hash 64 MB, and Move Overhead 30 ms. The artifact retains every final info line, external monotonic wall time, hard-deadline overshoot, nodes, reported time/NPS, depth, PV, binary hash, host, and schedule. Aggregate NPS uses final UCI nodes divided by final UCI time: the historical Threads-1 line describes its last completed iteration, while the new SMP final line contains exact all-worker work. External wall-NPS is diagnostic only. Retention requires at least 1.35x/1.70x median aggregate NPS for Threads 2/4 and at most 30 ms p95 hard-deadline overshoot for every configuration before paired games may start.

Frozen P3 passed this gate at `2.010776x / 3.956388x`; Threads 2/4 p95 hard-deadline overshoot was `0.675041 / 0.685708 ms`. The binding same-binary paired screen then completed all 2,000 games / 1,000 pairs at `0.5+0.005`: Threads 2 scored `711/701/588`, 53.075%, reported `+21.39 +/-10.90 Elo`, and pentanomial `[57,184,437,223,99]`. Independent parsing verified 1,000 unique paired FENs, equal colors, 196,758 telemetry-complete plies, and zero warning, timeout, forfeit, crash, illegal move, protocol error, or negative time-left sample. The complete artifact hashes and interpretation are in the experiment ledger. This retains P3 on the development branch; it does not change the released version or replace a separately preregistered normalized SPRT.

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

`match.sh` normally applies `THREADS` to both engines. A registered resource-scaling comparison may instead omit `THREADS` and set both `ENGINE_A_THREADS` and `ENGINE_B_THREADS`; partial or mixed shared/per-engine configuration is rejected. The runner records `thread_mode` and both effective values, so a same-binary Threads-2-versus-Threads-1 match remains independently auditable.

Keep engine settings equal, reverse colors, use an audited balanced opening suite, retain PGNs, and report games/W/D/L/score/Elo confidence intervals. The eight bundled openings are only a smoke/development set. The frozen 0.2.0 development configuration is recorded in [the baseline document](development/0.2.0-baseline.md), and feature outcomes belong in [the experiment ledger](development/experiments.md).

Audit a completed paired match independently of fastchess's final console report:

```bash
python3 -m venv .venv
.venv/bin/pip install -r scripts/requirements.txt
.venv/bin/python scripts/audit-match.py \
  --pgn testing/example/match.pgn \
  --log testing/example/match.log \
  --meta testing/example/match.meta.txt \
  --candidate NEYRANG-candidate \
  --opponent NEYRANG-parent \
  --expected-games 2000 \
  --expected-time-control 0.5+0.005
```

The auditor parses every game with python-chess, compares header and movetext results, reconstructs W/D/L and pentanomial counts, verifies consecutive opening pairs and color reversal, requires normal terminations and complete per-ply telemetry, reports latency/time-left distributions, checks completed metadata, scans the runner log for known failure classes, and compares its independent counts with fastchess's final summary. It exits nonzero on any mismatch.

The default warning policy is `reject-all` and is unchanged. A comparison against the immutable 0.2.0 binary may explicitly set `WARNING_POLICY=allow-opponent-threefold-pv` with `STRICT=0`, then pass the same policy to `audit-match.py`. This compatibility mode permits only an exact threefold-repetition PV-continuation warning from the registered opponent name. It retains and reports every permitted line; the same warning from the candidate, any other warning, or any ordinary anomaly still rejects the complete match. The runner refuses to combine this policy with fastchess strict mode because strict mode stops on the known baseline warning before the post-run audit can classify it.

P1 exercised this path with frozen G1 against immutable v0.2.0 over 2,000 games / 1,000 pairs at `0.5+0.005`. G1 scored `1006/623/371` (65.875%, reported `+114.26 +/-12.79 Elo`). Independent parsing verified 1,000 unique paired FENs, 196,771 telemetry-complete plies, 2,000 normal terminations, no negative time-left sample, and exact W/D/L/pentanomial reproduction. The log contained zero allowed warning and zero rejected warning or other anomaly: the compatibility path removed the runner ambiguity without excusing an event in the accepted match.

## Distributed fixed-game campaigns

Use the distributed tool when a pre-registered fixed number of paired games must
be split across workers. Preparation accepts frozen executables and a source EPD,
rejects canonical duplicates, hash-ranks the requested openings, and writes
host-independent manifests plus disjoint sequential shard books:

```bash
.venv/bin/python scripts/distributed-testing.py prepare \
  --engine-a /artifacts/neyrang-candidate \
  --engine-a-name NEYRANG-candidate \
  --engine-a-git-sha CANDIDATE_COMMIT \
  --engine-b /artifacts/neyrang-parent \
  --engine-b-name NEYRANG-parent \
  --engine-b-git-sha PARENT_COMMIT \
  --fastchess /artifacts/fastchess \
  --fastchess-version VERSION_OR_COMMIT \
  --openings /artifacts/openings.epd \
  --openings-source SOURCE \
  --openings-license LICENSE \
  --output testing/campaign-name \
  --seed 20260903 \
  --pairs 5000 \
  --pairs-per-shard 128 \
  --tc 0.5+0.005 \
  --hash-mb 64 \
  --threads 1 \
  --move-overhead-ms 100 \
  --time-margin-ms 0 \
  --strict
```

Preserve the printed `campaign_sha256` outside the campaign directory; it is the
worker's trust anchor. Transfer the unchanged campaign directory and frozen
executables to a matching OS/architecture, then run one registered shard:

```bash
.venv/bin/python scripts/distributed-testing.py run-shard \
  --manifest testing/campaign-name/shards/shard-0000.json \
  --campaign-sha256 CAMPAIGN_SHA256 \
  --engine-a /artifacts/neyrang-candidate \
  --engine-b /artifacts/neyrang-parent \
  --fastchess /artifacts/fastchess \
  --results worker-results
```

The worker refuses campaign, shard, platform, executable, fastchess, or opening
drift. It also refuses to overwrite a prior attempt. A failed shard keeps its
artifacts but emits no accepted result manifest; retry that whole shard in a new
results directory. Time-control components must be exactly representable in
milliseconds so the registered value round-trips through the PGN header.

After copying every worker artifact into one results directory, the coordinator
checks every result hash and reruns `audit-match.py` from raw PGN/log/metadata:

```bash
.venv/bin/python scripts/distributed-testing.py audit-campaign \
  --campaign testing/campaign-name/campaign.json \
  --campaign-sha256 CAMPAIGN_SHA256 \
  --results collected-results \
  --audit-script scripts/audit-match.py \
  --output collected-results/campaign-audit.json
```

This local protocol covers immutable fixed-game batches and exact aggregation.
It is influenced by OpenBench's frozen-workload model but does not implement the
OpenBench client/server API, dynamic work allocation, or a distributed SPRT/LLR
coordinator. Use a reviewed OpenBench deployment for that role; do not treat a
sequence of fixed shards as an SPRT by merely stopping when a point estimate looks
favorable.

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

## Post-0.3 F1 legality-filter evidence

The retained F1 implementation replaced production make/check/unmake legality filtering with final-occupancy validation while preserving the old generator as a test-only oracle. Exact list and order equality passed across 100,000 deterministic positions, including checks, double checks, pins, en passant, castling, and promotions. The normal/all-feature/tactical suites passed 64/77/11 tests; formatting, warnings-denied Clippy, the release build, all required Perft fixtures, match-runner tests, and fastchess UCI compliance 40/40 also passed.

All 15 interleaved depth-8 runs per frozen binary searched exactly 4,483,694 nodes with checksum `d8d4a9b6bbcde023`. Median time fell from 2,998 ms to 1,979 ms (-33.99%) and median NPS rose from 1,495,268 to 2,265,339 (+51.50%). The subsequent strict 2,000-game / 1,000-pair equal-time match at `0.5+0.005` scored 958 wins, 572 draws, and 470 losses for F1 (62.20%, reported `+86.52 +/-12.60 Elo`). Independent artifact auditing verified 1,000 unique paired FENs with colors reversed, all 195,161 plies with complete telemetry, 2,000 normal terminations, no negative time-left sample, and no timeout, warning, crash, illegal move, disconnect, forfeit, or protocol error. The full registration, hashes, and KEEP decision are in the [experiment ledger](development/experiments.md).

## Post-F1 G1 exact-SEE evidence

G1 carries the fixed-target attacker set and both color occupancies through each exact exchange, prepares the accepted legal LVA state once, and reveals only newly exposed slider x-rays. The complete pre-G1 implementation remains test-only. Exact score, threshold result, exchange-step count, and early-exit state matched over 100,000 legal positions, 336,083 captures/promotions, and 7,393,826 threshold queries. The normal/all-feature/tactical suites passed 65/78/11 tests; formatting, warnings-denied Clippy, warning-free release build, required Perft fixtures, match-runner tests, exact depth-5/depth-8 trees, and fastchess UCI compliance 40/40 also passed.

All 15 interleaved depth-8 runs per frozen binary retained exactly 4,483,694 nodes and checksum `d8d4a9b6bbcde023`. Median time fell from 2,341 to 2,237 ms (-4.44%) and median NPS rose from 1,915,076 to 2,003,511 (+4.62%). The subsequent strict 2,000-game / 1,000-pair screen at `0.5+0.005` scored 672 wins, 695 draws, and 633 losses for G1 (50.98%, reported `+6.78 +/-8.88 Elo`). Independent auditing verified all pairs and results, 1,000 unique opening FENs, all 196,468 plies with complete telemetry, 2,000 normal terminations, no negative time-left sample, and no timeout, warning, crash, illegal move, disconnect, forfeit, or protocol error. The full registration, artifacts, and KEEP decision are in the [experiment ledger](development/experiments.md).

## H0 evaluation-trace evidence

H0 adds only feature-gated measurement code. The normal release remains exactly 569,504 bytes with SHA-256 `056082e8e2e185a1b0a5bdaf78aaa54b4662414504cf7707891dc1d4350793e4`, byte-identical to frozen G1. The depth-5/depth-8 trees remain `180591/e04f83f9a9880115` and `4483694/d8d4a9b6bbcde023`.

The independent trace matched production evaluation and its own component reconstruction on 100,000 legal positions. Formatting, warnings-denied Clippy, 68 normal tests, 85 all-feature tests, 11 tactical tests, required Perft, both release builds, match-runner tests, and fastchess compliance 40/40 pass. Three one-million-row export runs produced the same checksum; median end-to-end wall time was 1.77 seconds on the development host. Full identities and the KEEP decision are in the [experiment ledger](development/experiments.md).

## H1 evaluation-corpus evidence

H1 added a deterministic pair-first corpus builder without changing the playing engine. Four Python tests cover opening-group co-location, deterministic reruns, quiet-position filtering, malformed/odd pair rejection, cross-partition transposition removal, manifest checksums, and no-overwrite behavior.

The historical pilot independently replayed 4,000 already-audited games / 2,000 color-reversed pairs and produced 2,967 globally unique quiet positions: 2,382 train, 318 validation, and 267 untouched holdout. Reversing source argument order produced byte-identical TSVs; every record exported as one fixed 38-column `neyrang-eval-trace-v1` row. A separate audit reconstructed every encoded ply and rechecked FEN, target, quietness, split assignment, and uniqueness without a mismatch. H1 is retained as evidence infrastructure only; the pilot is too small and correlated to authorize weight tuning.

## H2b fresh-corpus evidence

The replacement H2b source completed 4,000 fresh games / 2,000 unique color-reversed opening pairs at `0.5+0.005` in one uninterrupted run. All 392,304 plies contain the registered telemetry, every termination is normal, and the independent match audit reports zero timing, crash, legality, warning, or protocol failure. Exact binary, opening, runner, PGN, log, configuration, and metadata hashes are recorded in the experiment ledger.

The registered quiet extraction produced 3,002 paired samples and 2,910 globally unique positions after 92 within-partition duplicates: 2,347 train, 304 validation, and 259 sealed holdout. A deterministic rerun is byte-identical. `scripts/audit-eval-corpus.py` independently replays all 4,000 games, reconstructs every selected ply and FEN, repeats the split and deduplication, and compares each TSV byte for byte. Nine Python tests include detection of TSV tampering even when its manifest hash and byte count are maliciously updated. All feature exports have exactly 38 columns; only the holdout count, width, and checksum have been audited.

The default release remains byte-identical to frozen G1. Full Rust formatting, warnings-denied Clippy, normal/all-feature/tactical tests, match-runner tests, Perft, deterministic tree/checksum, and fastchess compliance 40/40 pass. H2b is retained as an eligible fresh source, but its group-level train/validation sample is being expanded before weight fitting.

The H2c expansion uses `scripts/select-fresh-openings.py` to reproduce a disjoint hash-ranked EPD from the source book plus PGN/EPD exclusions. Three tests cover order-independent determinism, both exclusion formats, canonical duplicates, insufficient source capacity, and refusal to overwrite. The 4,500-opening H2c artifact has no overlap with the historical or H2b opening sets; its exact registration and checksums are in the experiment ledger.

## H2c rejected expansion evidence

H2c completed 7,462 structurally valid games before fastchess recorded one F1 timeout in game 7,463 and exited nonzero. The independent audit reconstructs all 3,731 complete pairs and 732,065 telemetry-complete plies, but also confirms the timeout and missing completed metadata. The pre-registered all-or-nothing rule therefore rejects the entire H2c game source: it is not resumed, repaired, pooled, extracted, or used for fitting. Its artifacts remain only as independently reproducible failure evidence in the experiment ledger.

## H2d deterministic replacement evidence

The paired-match runner now supports exact per-engine node limits and refuses incomplete or mixed shared/per-engine configurations. The auditor can assert arbitrary metadata identities with repeatable `--expect-meta KEY=VALUE` arguments. Four new metadata tests bring the Python suite to 16 passing tests; the match/SPRT configuration suite also passes.

The registered H2d replacement uses G1 at 30,000 and F1 at 29,200 nodes per move, derived only from aggregate per-move H2c telemetry. A 20-game smoke run passed exact metadata, pairing, telemetry, strict-log, and process gates and diverged in 7 of 10 paired trajectories; its score is not used. The full restart then completed all 9,000 games / 4,500 pairs with 893,061 telemetry-complete plies, normal terminations, complete metadata, and zero warning, timeout, crash, disconnect, legality, or protocol anomaly. Three disclosed wall-clock latency outliers coincide with host sleep/suspension and cannot alter the registered node budgets. H2d is accepted only as a deterministic outcome-label source, not as time or strength evidence.

## H2e dense corpus and diagnostic evidence

H2e deterministically selects at most eight quiet positions per game with an eight-ply gap, then truncates both games in each color-reversed pair to equal record counts. H2b+H2d produced 38,234 pair-balanced records before deduplication and 35,032 unique records afterward: 28,040 train, 3,564 validation, and 3,428 current-holdout records. Reversing source order produced byte-identical TSVs, and the independent auditor replayed all 13,000 accepted source games and every selection/deduplication decision. The default one-position selector still reproduces the exact registered legacy TSV bytes.

The train/validation-only analyzer reconstructs all 42 effective evaluation columns and weights every opening pair equally. Both designs have full rank and no dead column. Scale-only fits are stable around `0.80-0.83`, and all validation point estimates improve slightly, but the registered 10,000-replicate group-bootstrap intervals include zero. H2e is retained as evidence infrastructure; no evaluation weight or playing source changes.

## N0a/N1a/N1b NNUE foundation evidence

`tools/nnue-reference` remains the scalar `Chess768` and network-artifact oracle. The following `tools/nnue-data` crate adds the canonical lossless game boundary without linking either crate into the playing engine. Its tests decode and re-encode the upstream Viriformat example byte for byte, replay every encoded move, cover castling/en-passant/underpromotion and concatenated games, and fail closed on corruption, truncation, illegal play or state that the strict subset cannot preserve.

`scripts/audit-nnue-data.py` is a second implementation over python-chess. It independently reconstructs the packed board, converts special moves, requires legality at each ply and reports content identity/count evidence. In strict self-play mode it also binds every packed initial position to the selected opening set, requires a registered terminal condition, and rejects WDL that disagrees with the final board. `write-smoke` generates three deterministic synthetic games that exercise all special-move types; these are codec evidence only, never a training or Elo sample.

The N1b recorder tests force checkmate and threefold trajectories, exercise score perspective/saturation, reject unfinished games at the maximum-ply boundary, validate every summary counter, and invoke the actual fixed-node NEYRANG search in a subprocess. The wrapper tests independently cover opening-manifest identity, partitioning of color-reversed groups, process progress, immutable publication, bad WDL, bad source counts/hashes, and overwrite refusal.

Run these layers with:

```bash
cargo fmt --manifest-path tools/nnue-data/Cargo.toml --check
cargo clippy --manifest-path tools/nnue-data/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path tools/nnue-data/Cargo.toml --locked
.venv/bin/python -m unittest \
  scripts.tests.test_audit_nnue_data \
  scripts.tests.test_generate_selfplay_shard
```

The exact record, provenance, rules-only completion policy and geometric 1M/4M/16M/64M/256M gates are frozen in the [NNUE data contract](development/nnue-data.md). A small self-play shard proves process and replay mechanics only; fixed-node data generation is not strength testing.

The first multi-thousand-game N1b infrastructure campaign used committed source
`8e761d1`, 4,096 fresh deterministic opening groups, 512 nodes per move and an
80/10/10 pre-generation split. Train and validation completed and independently
replayed all 3,683 attempted games with zero rejection, producing 320,947 scored
positions; all 413 holdout groups remained unplayed. Exact artifact, manifest
and membership hashes are recorded in the NNUE data contract. These games prove
recorder throughput and data integrity only: they neither train nor strengthen
the current engine and do not satisfy the one-million-position pipeline gate.

Run the deterministic Python gates with an environment containing the pinned dependencies:

```bash
python3 -m venv .venv-eval
.venv-eval/bin/pip install -r scripts/requirements-eval.txt
.venv-eval/bin/python -m unittest discover -s scripts/tests -p 'test_*.py'
```

The repository Python suite currently has 73 tests covering match/corpus auditing, corpus construction, dense pair balancing, legacy-manifest compatibility, opening selection, fixed 38-to-42 feature mapping, complete-group learning subsets, scale fitting, deterministic group bootstrap, repository contracts, scored-shard orchestration and independent NNUE-record/terminal replay. Exact campaign, corpus, feature, diagnostic, and script hashes are recorded in the experiment ledger.
