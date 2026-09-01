# NEYRANG

NEYRANG is an independent UCI chess engine written from scratch in stable Rust.
It is the new identity of the project formerly called NEYRANG. The latest tagged
release remains the legacy `NEYRANG 0.2.0`; the current development identity is
`NEYRANG 0.3.0-dev`. The `dev/0.3-search` branch contains accepted but
unreleased timing, staged-MovePicker, threshold-SEE, conservative-NMP, and
non-mutating legal-generation work developed through isolated correctness
gates, deterministic benchmarks, paired engine matches, and SPRT.

The four project names are executable architecture boundaries, not decorative
labels: [NEYRANG, REKHNE, SANJ, and SHEGERD](docs/naming.md).

NEYRANG does not wrap an existing chess library or engine. The runtime chess core uses only the Rust standard library.

## Current status

Implemented:

- 64-bit bitboards plus a synchronized mailbox (`a1 = 0`, `h8 = 63`)
- FEN parse/serialize with recoverable validation errors
- compile-time pawn, knight, and king attacks; portable ray-based sliders
- complete legal move generation, including castling, en passant, and underpromotions
- reversible make/unmake and deterministic incremental Zobrist hashing
- fixed-capacity move lists with no per-node heap allocation
- occupancy-only legality filtering on the development branch, with the old
  make/check/unmake generator retained as a test oracle
- classical tapered evaluation
- iterative deepening, alpha-beta, quiescence, PVS, aspiration windows
- transposition table with mate-score normalization
- a full-key local TT for `Threads=1` and a coherent packed atomic shared TT
  for retained Lazy SMP on the development branch
- TT/SEE-capture/killer/history move ordering with losing captures deferred
- fixed-storage staged move delivery on the development branch
- legal static exchange evaluation and conservative qsearch SEE pruning
- conservative one-ply late move reductions with mandatory full-depth re-search
- guarded fixed-R2 null-move pruning on the development branch
- 50-move, threefold repetition, checkmate, and stalemate detection
- cooperative atomic stop plus soft/hard time limits
- asynchronous UCI loop, root-diversified Lazy SMP, and deterministic benchmark command
- feature-gated exact evaluation trace and streaming dataset export on the development branch
- deterministic pair-first train/validation/holdout corpus construction with provenance and leakage controls
- deterministic dense, gap-constrained, pair-balanced corpus extraction while preserving the legacy selector
- independent PGN-to-corpus replay auditing, including resistance to rehashed TSV tampering
- opening-group-aware train/validation diagnostics for the registered 42-parameter evaluation design
- deterministic disjoint-opening selection from audited EPD/PGN provenance sets
- immutable fixed-game campaign sharding with per-worker asset verification,
  exact opening-sequence audits, checksummed result manifests, and coordinator replay
- isolated scalar NNUE reference tooling with versioned fail-closed artifacts,
  dual-perspective feature mapping, and refresh/incremental accumulator oracles
- feature-gated SANJ NNUE engine inference with an independent scalar
  implementation, bit-exact engine/reference parity, move-delta accumulators,
  explicit UCI `EvalFile` activation, Lazy-SMP worker ownership, and a separate
  end-to-end NNUE benchmark
- isolated lossless NNUE game records with legal replay, byte-exact upstream
  compatibility fixtures, special-move coverage, and an independent auditor
- deterministic OpenBench `genfens` opening generation using every bit of the
  unsigned 64-bit seed, with shard-invariant streams and exact protocol output
- independently audited, content-addressed opening shards with executable,
  source, compiler, and license provenance in adjacent manifests
- isolated fixed-node self-play recording with white-relative parent scores,
  rules-only WDL, pre-generation opening-group splits, reason counts, immutable
  scored-shard manifests, and independent terminal/WDL replay auditing
- deterministic complete-game NNUE corpus assembly with source-policy binding,
  canonical position-key leakage detection, whole-game quarantine, immutable
  game shuffling, and independently replayed output manifests
- a Bullet trainer pinned to an exact revision, a deterministic position reader,
  fail-closed quantized export into the `NEYRANG\0` artifact format, and independent
  raw-float/quantized FEN parity, full-corpus diagnostics, and paired
  fixed-validation game bootstrap comparison
- independent dual-perspective king-square exposure analysis for data-backed
  king-bucket architecture registration

Not implemented in the default playing source: Syzygy, enabled-by-default NNUE,
LMP, futility pruning, continuation/capture history, or an optimized
sliding-attack backend. The NNUE lane has an accepted experimental
sixteen-million-position learning-curve artifact and a feature-gated N2 scalar
engine path. The frozen 16M network failed its first 1,000-game fixed-node
playing screen at 27.20% and is rejected; the default evaluator therefore
remains classical and no NNUE network or Elo claim is part of a release. The
single pre-registered P3
Lazy-SMP design passed its correctness, scaling, deadline, and 2,000-game paired
gates and is retained on the development branch. This is controlled development
evidence, not a new release or a universal Elo claim. Capture History and 1-ply
Continuation History were tested and reverted; the second conservative NMP
experiment was retained on the development branch but has not earned a release.

N1e has closed the 16M learning-curve gate: four frozen campaigns produced an
18,140,767-position train corpus that is disjoint from the unchanged
463,228-position N1d validation corpus. Nine duplicated-opening games and 298
validation-overlapping train games were quarantined as whole games; residual
opening and cross-partition position overlap are zero. The exact Metal epoch
consumed a registered 16,008,492-position prefix, passed both parity suites and
lowered fixed-validation MSE from `0.076680656365` to `0.074149893964`. Its
pre-registered paired game bootstrap interval is wholly below zero. All 25,715
assigned holdout openings remain unplayed and are not a final model-selection
holdout. This is data/training evidence, not a playing-strength result; exact
identities are in the
[NNUE data contract](docs/development/nnue-data.md) and
[N1e evidence manifest](docs/evidence/nnue-n1e-16m.json).

No project license has been selected yet.

## Requirements and build

NEYRANG pins the stable channel and requires Rust 1.98 or newer with Edition 2024.

```bash
xcode-select --install
rustup update stable
cargo build --release
```

Portable release binary:

```text
target/release/neyrang
```

Experimental NNUE builds are opt-in and do not embed a network:

```bash
cargo build --release --features nnue
target/release/neyrang bench-nnue /path/to/registered-network.nnue 5
```

In UCI, set `EvalFile` only to a separately audited `NEYRANG\0` artifact. An
empty value restores classical SANJ. The loader rejects legacy magic,
incompatible shape/version/quantization, truncation, trailing bytes, and CRC
mismatch. Match runners record the full network SHA-256 independently.

OpenBench-compatible build (the executable is written at the repository root
under the exact `EXE` name):

```bash
make EXE=NEYRANG-local
./NEYRANG-local bench
scripts/test-openbench-contract.sh
```

Apple Silicon native tuning is optional and should not be used for portable artifacts:

```bash
RUSTFLAGS="-C target-cpu=native" cargo build --profile maxperf
```

## UCI usage

Run `target/release/neyrang` without arguments and configure it as a UCI engine in a chess GUI. Supported commands include `uci`, `isready`, `ucinewgame`, `position`, `setoption`, `go`, `stop`, and `quit`.

Smoke test:

```bash
printf "uci\nisready\nposition startpos\ngo depth 5\nquit\n" | target/release/neyrang
```

Options:

- `Hash` (default 64 MB)
- `Threads` (default 1; values above one enable the retained shared-TT
  Lazy-SMP path on the development branch)
- `Move Overhead` (default 30 ms; configurable)
- `EvalFile` (available only in an `nnue` feature build; empty/classical by
  default)

## En Croissant

NEYRANG can be loaded directly as a local UCI engine in En Croissant. On macOS, build a stable versioned executable with:

```bash
scripts/en-croissant.sh prepare
```

Select the printed path from En Croissant's **Engines** page. The preserved
legacy NEYRANG protocol-smoke PGN can then be opened with:

```bash
scripts/en-croissant.sh open
```

See [the En Croissant integration guide](docs/en-croissant.md) for engine registration, arbitrary PGNs, and reproducible game generation.

## Perft

```bash
target/release/neyrang perft 5
target/release/neyrang divide 4
target/release/neyrang perft-fen "<FEN>" 4
```

Verified release results:

| Position | Depth | Nodes |
| --- | ---: | ---: |
| Initial position | 5 | 4,865,609 |
| Canonical Kiwipete | 4 | 4,085,603 |
| Rook/pawn endgame | 5 | 674,624 |

The similarly named FEN `r3k2r/p1ppqpb1/bn2pnp1/2pP4/1p2P3/2N2N2/PPQBBPPP/R3K2R w KQkq - 0 1` has 45 legal root moves, not 48. That result was independently cross-checked; the canonical 48-move Kiwipete fixture is kept separately in the tests.

## Benchmark

```bash
target/release/neyrang bench
RUNS=5 scripts/bench.sh
```

The benchmark searches five fixed positions at depth 5 and reports deterministic nodes and checksum plus machine-dependent time/NPS. See [testing documentation](docs/testing.md) for the recorded baseline and measurement caveats.

The `0.2.0` search visits 196,627 nodes with checksum `4a4c31e290740db3` on this workload, down from 448,136 nodes in `0.1.0`. Its final normalized SPRT against the immutable `v0.1.0` binary accepted H1 after 786 games; the full configuration and the reported estimate are recorded in the [experiment ledger](docs/development/experiments.md). This controlled result is relative to that exact baseline and time control, not a universal Elo claim.

## Tests and engine matches

```bash
cargo fmt --check
cargo clippy --all-targets --all-features
cargo test
cargo test --all-features
cargo test --features stats
cargo test --manifest-path tools/nnue-reference/Cargo.toml --locked
cargo test --manifest-path tools/nnue-data/Cargo.toml --locked
scripts/test-match-config.sh
.venv/bin/python -m unittest discover -s scripts/tests
```

For paired fastchess testing:

```bash
ENGINE_B=/path/to/parent/neyrang GAMES=400 scripts/regression.sh
ENGINE_A=target/release/neyrang ENGINE_B=/path/to/opponent scripts/match.sh
ENGINE_B=/path/to/parent/neyrang scripts/sprt.sh
```

For fixed-size campaigns that must be split across machines, use
`scripts/distributed-testing.py`. It hash-ranks a unique opening set into disjoint
color-reversed shards, verifies the campaign SHA and every executable before a
worker starts, and requires a second raw-PGN audit when the coordinator combines
results. The complete command contract and the boundary with a real OpenBench
SPRT server are documented in [testing](docs/testing.md#distributed-fixed-game-campaigns).

The repository also satisfies the local OpenBench build/bench contract. A real
distributed SPRT deployment still requires a selected license, public remote,
pinned server fork, audited book, and trusted workers; see the
[OpenBench integration guide](docs/openbench.md).

OpenBench-compatible deterministic openings can be generated directly:

```bash
target/release/neyrang "genfens 8 seed 2026090100000001 book None" quit
```

Retained local shards must go through the independent wrapper so the process is
monitored, every FEN is revalidated with `python-chess`, and an immutable
manifest is written beside the EPD:

```bash
.venv/bin/python scripts/generate-opening-shard.py \
  --engine target/release/neyrang \
  --output testing/nnue/openings/shard-0000.epd \
  --count 256 \
  --seed 2026090100000001 \
  --generator-source-commit FULL_GIT_SHA \
  --compiler-identity "rustc VERSION" \
  --source-license UNLICENSED-NEYRANG-INTERNAL
```

The license value above is an explicit internal-data marker, not permission to
redistribute a corpus. Select a real project/data license before publication.

Scored self-play must likewise use the audited wrapper rather than invoking the
Rust recorder as a retained-data shortcut:

```bash
cargo build --release --manifest-path tools/nnue-data/Cargo.toml \
  --bin generate-selfplay
.venv/bin/python scripts/generate-selfplay-shard.py \
  --generator tools/nnue-data/target/release/generate-selfplay \
  --openings testing/nnue/openings/shard-0000.epd \
  --openings-manifest testing/nnue/openings/shard-0000.epd.manifest.json \
  --output testing/nnue/selfplay/train-0000.vf \
  --partition train \
  --split-seed 20260901-n1b-v1 \
  --nodes 2000 --hash-mb 16 --max-plies 512 \
  --generator-source-commit FULL_GIT_SHA \
  --compiler-identity "rustc VERSION" \
  --source-license UNLICENSED-NEYRANG-INTERNAL
```

Assemble complete games only after every input shard is frozen and audited:

```bash
.venv/bin/python scripts/assemble-nnue-corpus.py \
  --input testing/nnue/selfplay/train-0000.vf \
  --input testing/nnue/selfplay/train-0001.vf \
  --output testing/nnue/corpus/train.vf \
  --partition train \
  --shuffle-seed DATASET_SHUFFLE_SEED \
  --min-games EXPECTED_GAMES \
  --min-scored-positions EXPECTED_POSITIONS
```

Validation adds `--disjoint-from testing/nnue/corpus/train.vf`; an explicit
`--quarantine-cross-partition-games` removes the complete conflicting validation
game and records every removed game/position. It never removes an individual
position. Duplicate opening groups fail closed by default; the explicit
`--quarantine-duplicate-opening-games` policy deterministically retains one
complete game per group and records every whole-game removal.

The retained 16M Metal epoch uses the pinned trainer crate, exact complete-batch
geometry and the already shuffled/audited corpus:

```bash
cargo run --release --manifest-path tools/nnue-trainer/Cargo.toml -- \
  --train testing/nnue/corpus/train.vf \
  --output-dir testing/nnue/checkpoints \
  --net-id neyrang-n1e-16m \
  --positions 16008492 --batch-size 9894 --buffer-mb 8 --threads 1 \
  --position-filter none --wdl-proportion 0.75 --feature-set chess768
```

Import and compare the exact raw/quantized tensors with explicit scale values:

```bash
cargo run --release --manifest-path tools/nnue-reference/Cargo.toml \
  --bin import-bullet -- BULLET_QUANTISED.bin NEYRANG_NETWORK.nnue 511 768 400
cargo run --release --manifest-path tools/nnue-reference/Cargo.toml \
  --bin verify-parity -- BULLET_RAW.bin NEYRANG_NETWORK.nnue FENS.txt 8 2 --summary-only
```

The N2c king-relative candidate uses `--feature-set chess768x3hm`. Bullet's
checkpoint `raw.bin` intentionally omits save-format transforms, so factorised
runs first export `merged-raw.bin` from `optimiser_state/weights.bin` with
`export-factorised-raw`; quantization screening and parity must use that merged
file. Pass the feature option to `import-bullet` and `screen-quantization`.
`verify-parity` derives the feature contract from the versioned artifact and
fails closed if the raw tensor shape disagrees.

These commands create data and offline-learning evidence, not Elo evidence. The
64M/256M scale points, final model-selection holdout, engine parity, playing
integration, speed tests and all game gates remain open.

`OPENING_SEED`, `OPENING_ORDER`, `TC`, `HASH_MB`, `THREADS`, and `CONCURRENCY` are explicit inputs. Set `NODES` on `match.sh` for a node-limited comparison; omit it for a time-controlled match. Each run writes PGN telemetry plus adjacent `.log` and `.meta.txt` files containing engine, Git, binary, opening, and fastchess identities. Pass `ENGINE_A_GIT_SHA`, `ENGINE_B_GIT_SHA`, `OPENINGS_SOURCE`, and `OPENINGS_LICENSE` for an auditable experiment.

The bundled opening file is intentionally small; replace `OPENINGS_FILE` with a larger audited balanced suite for strength testing. Do not infer Elo from Perft, NPS, tactical puzzles, or a small game sample.

## Architecture

- [Architecture](docs/architecture.md)
- [Naming and migration contract](docs/naming.md)
- [REKHNE search strategy](docs/rekhne.md)
- [SANJ position judgment](docs/sanj.md)
- [Testing and benchmarks](docs/testing.md)
- [OpenBench integration](docs/openbench.md)
- [En Croissant integration](docs/en-croissant.md)
- [0.2.0 frozen baseline](docs/development/0.2.0-baseline.md)
- [Search experiment ledger](docs/development/experiments.md)
- [0.3 retrospective game campaign](docs/development/0.3.0-game-campaign.md)
- [0.3 timing audit and BASE_0_3](docs/development/0.3.0-timing-audit.md)
- [0.3.0 search plan](docs/development/0.3.0-plan.md)
- [0.3 cumulative release validation](docs/development/0.3.0-release-validation.md)
- [0.3 search-development final report](docs/development/0.3.0-final-report.md)
- [Next strength phase and profile](docs/development/next-strength-phase.md)
- [Competitive roadmap: OpenBench, H3, NNUE, and selective search](docs/development/competitive-roadmap.md)
- [NNUE scalar reference and artifact contract](docs/development/nnue-reference.md)
- [NNUE lossless game-record and provenance contract](docs/development/nnue-data.md)
- [SANJ corpus and tuning protocol](docs/development/sanj-tuning.md)

## Contributing and security

Evidence and review requirements are documented in [CONTRIBUTING.md](CONTRIBUTING.md).
Report security-sensitive parser, resource, build, or artifact-integrity issues
using the process in [SECURITY.md](SECURITY.md). The project owner has not yet
selected a license, so source availability is not currently an open-source
license grant.

## Roadmap

The legacy NEYRANG 0.3 search order was evidence-driven. A completed 4,000-game
retrospective found direct positive evidence for SEE ordering and qsearch
pruning, an inconclusive isolated LMR result, and `+90.97 +/-18.38 Elo` for
cumulative v0.2.0 against v0.1.0. A separate timing audit retained isolated
hardening after a clean 1,000-game stress. The staged MovePicker then earned
retention through 7,046 valid comparison games. The following lazy main-search
`see_ge` caller passed deterministic and two 1,000-game screens, but its fresh
10,000-game SPRT capped inconclusively and was reverted; the verified threshold
primitive remains. Capture History failed its binding deterministic efficiency
rule, and 1-ply Continuation History was reverted after a strict candidate
timeout. Conservative fixed-R2 NMP passed correctness, accepted H1 after 4,946
fresh SPRT games, and scored 54.95% over 1,000 clean games at `8+0.08`; it was
retained after 7,946 valid comparison games. The cumulative candidate was 8.16%
smaller by nodes and 11.97% faster than v0.2.0 in the interleaved benchmark, but
two strict primary-screen attempts were interrupted by reproducible
repetition-PV defects in immutable v0.2.0. The protocol allowed no third attempt,
so no legacy v0.3.0 tag was created. The renamed development identity is
`NEYRANG 0.3.0-dev`; this rename itself carries no new strength claim.

Post-campaign F1 removed make/unmake from the common legality-filtering path without changing the legal list, move order, benchmark tree, or checksum. Its 15-run interleaved depth-8 benchmark reduced median wall time by 33.99% and raised median NPS by 51.50%. A separately scored strict 2,000-game match then finished `958/572/470` from the candidate perspective, 62.20%, with no timing, legality, crash, or protocol anomaly.

G1 then made exact SEE carry target attackers and color occupancy incrementally while retaining the complete pre-G1 implementation as a test oracle. Equality passed over 100,000 positions, 336,083 tactical moves, and 7,393,826 threshold queries. The tree-identical 15-run depth-8 comparison reduced median wall time by another 4.44%; a fresh strict 2,000-game screen finished `672/695/633` (50.98%) with all 2,000 terminations normal and no timing, legality, crash, or protocol anomaly. G1 is retained at playing-source commit `b80b08a2e188973383d254dd6b3ced908560364c`.

P1 then resolved the immutable-v0.2.0 runner ambiguity with a narrow, independently tested opponent-only repetition-PV warning policy. A fresh 2,000-game cumulative screen completed `1006/623/371` for G1 (65.875%, reported `+114.26 +/-12.79 Elo`). Independent replay verified every pair and all 196,771 plies, with 2,000 normal terminations and zero warning, timeout, crash, illegal move, protocol error, or negative time-left sample.

P3 subsequently retained one root-diversified Lazy-SMP design. Frozen Threads 2/4 throughput reached `2.010776x / 3.956388x` the Threads-1 aggregate NPS with sub-millisecond p95 hard-deadline overshoot. The binding same-binary Threads-2-versus-Threads-1 screen completed all 2,000 games at `0.5+0.005`: `711/701/588`, 53.075%, reported `+21.39 +/-10.90 Elo`, and pentanomial `[57,184,437,223,99]`. Independent replay verified all 1,000 pairs and 196,758 plies with zero warning, timeout, crash, illegal move, protocol error, forfeit, or negative time-left sample. Including the independently audited N2a rejection screen, the project has now recorded 44,992 valid comparison games. Accepted work stays on `dev/0.3-search`; the released version and immutable `v0.2.0` tag remain unchanged. A normalized Lazy-SMP SPRT requires its own preregistration. Pawn hashing remains lower priority.

H0/H1 then added byte-isolated evaluation evidence tooling and proved it on a 4,000-game historical pilot. H2b subsequently completed 4,000 fresh paired games without a timing, crash, legality, warning, or protocol failure. The first 9,000-game H2c expansion was rejected in full after one timeout; its deterministic H2d replacement completed and independently audited all 9,000 games at fixed per-engine node budgets. H2b+H2d first produced a conservative 9,959-record corpus, then H2e recovered 35,032 unique records through pre-registered gap-constrained, pair-balanced dense extraction without generating or accepting another game. Train and validation contain 28,040/3,564 rows from 4,146/538 opening groups, both 42-column designs have full rank, and a scale-only diagnostic is stable but statistically unresolved on validation. No evaluation weight changed. Because aggregate current-holdout summaries were accidentally exposed, any future fitter requires its own preregistration and a new disjoint untouched final holdout before a playing candidate can exist.

N0a/N1a/N1b/N1c/N1d/N1e cover the data and learning-curve path from scalar `Chess768` inference and strict Viriformat games through deterministic corpus assembly, pinned Bullet ingestion, quantized export, fixed FEN parity and paired fixed-validation diagnostics. The 16M point retained 18,140,767 train positions and consumed an exact 16,008,492-position epoch against the unchanged 463,228-position validation corpus. The selected quantization stays within the preregistered `8 cp` maximum and `2 cp` mean error bounds, and all three N1e validation gates passed. N2a now adds an opt-in scalar engine bridge and worker-owned accumulator stack with bit-exact reference parity. Fifteen interleaved benchmarks were deterministic, then an independently audited 1,000-game equal-node screen rejected the 16M network at `177/633/190` and 27.20% (`-171.02 +/- 20.09` reported logistic Elo). That is evidence against this network, not evidence against the inference path. The final holdout does not yet exist, the default engine remains classical, and SIMD or any 64M extension requires a newly registered model/data diagnosis rather than automatic scale growth. Exact N2a evidence is in [the engine-inference manifest](docs/evidence/nnue-n2a-engine-inference.json).
