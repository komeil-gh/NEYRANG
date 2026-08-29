# NEYRANG

NEYRANG is an independent UCI chess engine written from scratch in stable Rust. Version `0.2.0` is a correct, single-threaded, classical engine developed through isolated search experiments: Perft and tactical correctness first, then deterministic benchmarks, paired engine matches, and SPRT.

NEYRANG does not wrap an existing chess library or engine. The runtime chess core uses only the Rust standard library.

## Current status

Implemented:

- 64-bit bitboards plus a synchronized mailbox (`a1 = 0`, `h8 = 63`)
- FEN parse/serialize with recoverable validation errors
- compile-time pawn, knight, and king attacks; portable ray-based sliders
- complete legal move generation, including castling, en passant, and underpromotions
- reversible make/unmake and deterministic incremental Zobrist hashing
- fixed-capacity move lists with no per-node heap allocation
- classical tapered evaluation
- iterative deepening, alpha-beta, quiescence, PVS, aspiration windows
- transposition table with mate-score normalization
- TT/SEE-capture/killer/history move ordering with losing captures deferred
- legal static exchange evaluation and conservative qsearch SEE pruning
- conservative one-ply late move reductions with mandatory full-depth re-search
- 50-move, threefold repetition, checkmate, and stalemate detection
- cooperative atomic stop plus soft/hard time limits
- asynchronous UCI loop and deterministic benchmark command

Not implemented yet: SMP, Syzygy, NNUE, null-move pruning, LMP, futility pruning, continuation/capture history, or an optimized sliding-attack backend. `Threads` is accepted by UCI but search remains deliberately single-threaded. A guarded null-move implementation was tested and deliberately reverted because its capped SPRT did not accept the positive hypothesis.

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

## Roadmap

The 0.3 search order is evidence-driven. A completed 4,000-game retrospective found direct positive evidence for SEE ordering and qsearch pruning, an inconclusive isolated LMR result, and `+90.97 +/-18.38 Elo` for cumulative v0.2.0 against v0.1.0. A separate timing audit retained isolated hardening after a clean 1,000-game stress. The staged MovePicker then earned retention through 7,046 valid comparison games. The following lazy main-search `see_ge` caller passed deterministic and two 1,000-game screens, but its fresh 10,000-game SPRT capped inconclusively at `+0.69` LLR and was reverted by `2e9637e`; the verified threshold primitive remains. Development now proceeds to Capture History, then 1-ply Continuation History. Guarded null-move pruning may be reconsidered only after those ordering experiments establish a new baseline. Pawn hashing remains lower priority. Every candidate is isolated and must earn retention through correctness gates, benchmarks, paired games, and SPRT when warranted.
