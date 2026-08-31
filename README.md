# NEYRANG

NEYRANG is an independent UCI chess engine written from scratch in stable Rust. Version `0.2.0` remains the latest release. The `dev/0.3-search` branch contains accepted but unreleased timing, staged-MovePicker, threshold-SEE, conservative-NMP, and non-mutating legal-generation work developed through isolated correctness gates, deterministic benchmarks, paired engine matches, and SPRT.

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
- TT/SEE-capture/killer/history move ordering with losing captures deferred
- fixed-storage staged move delivery on the development branch
- legal static exchange evaluation and conservative qsearch SEE pruning
- conservative one-ply late move reductions with mandatory full-depth re-search
- guarded fixed-R2 null-move pruning on the development branch
- 50-move, threefold repetition, checkmate, and stalemate detection
- cooperative atomic stop plus soft/hard time limits
- asynchronous UCI loop and deterministic benchmark command
- feature-gated exact evaluation trace and streaming dataset export on the development branch
- deterministic pair-first train/validation/holdout corpus construction with provenance and leakage controls
- deterministic dense, gap-constrained, pair-balanced corpus extraction while preserving the legacy selector
- independent PGN-to-corpus replay auditing, including resistance to rehashed TSV tampering
- opening-group-aware train/validation diagnostics for the registered 42-parameter evaluation design
- deterministic disjoint-opening selection from audited EPD/PGN provenance sets
- immutable fixed-game campaign sharding with per-worker asset verification,
  exact opening-sequence audits, checksummed result manifests, and coordinator replay

Not implemented in the retained development source: SMP, Syzygy, NNUE, LMP, futility pruning, continuation/capture history, or an optimized sliding-attack backend. `Threads` is accepted by UCI but search remains deliberately single-threaded. Capture History and 1-ply Continuation History were tested and reverted; the second conservative NMP experiment was retained on the development branch but has not earned a release.

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
- `Threads` (accepted; current implementation uses one search thread)
- `Move Overhead` (default 30 ms; configurable)

## En Croissant

NEYRANG can be loaded directly as a local UCI engine in En Croissant. On macOS, build a stable versioned executable with:

```bash
scripts/en-croissant.sh prepare
```

Select the printed path from En Croissant's **Engines** page. A bundled PGN of the complete paired smoke games can then be opened with:

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
cargo test --features stats
scripts/test-match-config.sh
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

`OPENING_SEED`, `OPENING_ORDER`, `TC`, `HASH_MB`, `THREADS`, and `CONCURRENCY` are explicit inputs. Set `NODES` on `match.sh` for a node-limited comparison; omit it for a time-controlled match. Each run writes PGN telemetry plus adjacent `.log` and `.meta.txt` files containing engine, Git, binary, opening, and fastchess identities. Pass `ENGINE_A_GIT_SHA`, `ENGINE_B_GIT_SHA`, `OPENINGS_SOURCE`, and `OPENINGS_LICENSE` for an auditable experiment.

The bundled opening file is intentionally small; replace `OPENINGS_FILE` with a larger audited balanced suite for strength testing. Do not infer Elo from Perft, NPS, tactical puzzles, or a small game sample.

## Architecture

- [Architecture](docs/architecture.md)
- [Search](docs/search.md)
- [Evaluation](docs/evaluation.md)
- [Testing and benchmarks](docs/testing.md)
- [En Croissant integration](docs/en-croissant.md)
- [0.2.0 frozen baseline](docs/development/0.2.0-baseline.md)
- [Search experiment ledger](docs/development/experiments.md)
- [0.3 retrospective game campaign](docs/development/0.3.0-game-campaign.md)
- [0.3 timing audit and BASE_0_3](docs/development/0.3.0-timing-audit.md)
- [0.3.0 search plan](docs/development/0.3.0-plan.md)
- [0.3 cumulative release validation](docs/development/0.3.0-release-validation.md)
- [0.3 search-development final report](docs/development/0.3.0-final-report.md)
- [Next strength phase and profile](docs/development/next-strength-phase.md)
- [Evaluation corpus and tuning protocol](docs/development/evaluation-tuning.md)

## Roadmap

The 0.3 search order is evidence-driven. A completed 4,000-game retrospective found direct positive evidence for SEE ordering and qsearch pruning, an inconclusive isolated LMR result, and `+90.97 +/-18.38 Elo` for cumulative v0.2.0 against v0.1.0. A separate timing audit retained isolated hardening after a clean 1,000-game stress. The staged MovePicker then earned retention through 7,046 valid comparison games. The following lazy main-search `see_ge` caller passed deterministic and two 1,000-game screens, but its fresh 10,000-game SPRT capped inconclusively and was reverted; the verified threshold primitive remains. Capture History failed its binding deterministic efficiency rule, and 1-ply Continuation History was reverted after a strict candidate timeout. Conservative fixed-R2 NMP passed correctness, accepted H1 after 4,946 fresh SPRT games, and scored 54.95% over 1,000 clean games at `8+0.08`; it was retained after 7,946 valid comparison games. The cumulative 0.3 candidate was 8.16% smaller by nodes and 11.97% faster than v0.2.0 in the interleaved benchmark, but two strict primary-screen attempts were interrupted by reproducible repetition-PV defects in immutable v0.2.0. The pre-registered protocol allowed no third attempt, so NEYRANG remains version 0.2.0 and no v0.3.0 tag was created.

Post-campaign F1 removed make/unmake from the common legality-filtering path without changing the legal list, move order, benchmark tree, or checksum. Its 15-run interleaved depth-8 benchmark reduced median wall time by 33.99% and raised median NPS by 51.50%. A separately scored strict 2,000-game match then finished `958/572/470` from the candidate perspective, 62.20%, with no timing, legality, crash, or protocol anomaly.

G1 then made exact SEE carry target attackers and color occupancy incrementally while retaining the complete pre-G1 implementation as a test oracle. Equality passed over 100,000 positions, 336,083 tactical moves, and 7,393,826 threshold queries. The tree-identical 15-run depth-8 comparison reduced median wall time by another 4.44%; a fresh strict 2,000-game screen finished `672/695/633` (50.98%) with all 2,000 terminations normal and no timing, legality, crash, or protocol anomaly. G1 is retained at playing-source commit `b80b08a2e188973383d254dd6b3ced908560364c`.

P1 then resolved the immutable-v0.2.0 runner ambiguity with a narrow, independently tested opponent-only repetition-PV warning policy. A fresh 2,000-game cumulative screen completed `1006/623/371` for G1 (65.875%, reported `+114.26 +/-12.79 Elo`). Independent replay verified every pair and all 196,771 plies, with 2,000 normal terminations and zero warning, timeout, crash, illegal move, protocol error, or negative time-left sample. The project has now recorded 41,992 valid comparison games. Accepted work stays on `dev/0.3-search`; the released version and immutable `v0.2.0` tag remain unchanged. Pawn hashing remains lower priority.

H0/H1 then added byte-isolated evaluation evidence tooling and proved it on a 4,000-game historical pilot. H2b subsequently completed 4,000 fresh paired games without a timing, crash, legality, warning, or protocol failure. The first 9,000-game H2c expansion was rejected in full after one timeout; its deterministic H2d replacement completed and independently audited all 9,000 games at fixed per-engine node budgets. H2b+H2d first produced a conservative 9,959-record corpus, then H2e recovered 35,032 unique records through pre-registered gap-constrained, pair-balanced dense extraction without generating or accepting another game. Train and validation contain 28,040/3,564 rows from 4,146/538 opening groups, both 42-column designs have full rank, and a scale-only diagnostic is stable but statistically unresolved on validation. No evaluation weight changed. Because aggregate current-holdout summaries were accidentally exposed, any future fitter requires its own preregistration and a new disjoint untouched final holdout before a playing candidate can exist.
