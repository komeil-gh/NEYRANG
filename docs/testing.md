# Testing NEYRANG

This guide covers the public, reproducible checks for the engine. Keep private
datasets, machine-specific paths, host details, and raw experiment output out
of the repository.

## Requirements

- Rust 1.98 or newer
- Python dependencies from `scripts/requirements.txt` for script tests
- A native linker

## Core checks

Run these commands from the repository root:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --locked --all-features
scripts/test-match-config.sh
```

For the Python utilities:

```bash
python3 -m venv .venv
.venv/bin/pip install -r scripts/requirements.txt
.venv/bin/python -m unittest discover -s scripts/tests
```

## Perft

Perft validates legal move generation by counting leaf nodes:

```bash
cargo run --release --locked -- perft 5
cargo run --release --locked -- divide 4
```

Reference counts:

| Position | Depth | Nodes |
| --- | ---: | ---: |
| Initial position | 5 | 4,865,609 |
| Canonical Kiwipete | 4 | 4,085,603 |
| Rook/pawn endgame | 5 | 674,624 |

The exact FEN positions are covered by the Rust integration tests.

## UCI smoke test

```bash
cargo build --release --locked
printf 'uci\nisready\nposition startpos\ngo depth 4\nquit\n' \
  | target/release/neyrang
```

A successful run prints `uciok`, `readyok`, search information, and a legal
`bestmove`.

## Benchmark

```bash
cargo run --release --locked -- bench
```

Node counts and checksums are deterministic for a given engine version and
workload. Timing and nodes per second depend on the machine, operating system,
compiler, and build flags.

## Match testing

Use balanced openings, color-reversed pairs, equal resource limits, frozen
binaries, and a predeclared acceptance rule. Record game count, W/D/L,
pentanomial results, time control, engine options, and abnormal terminations.

Do not treat Perft, tactical positions, node reductions, NPS, or offline model
loss as Elo evidence.

## Private artifacts

Generated games, corpora, networks, profiles, machine inventories, absolute
paths, credentials, and host-specific launch files belong under ignored local
directories such as `testing/private/`, never in Git.
