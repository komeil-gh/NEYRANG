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

## Native profile-guided build

PGO is optional deployment work; it does not replace the portable release.
Build an instrumented native binary into a new profile directory:

```bash
scripts/build-native-pgo.sh instrument testing/private/pgo \
  testing/private/neyrang-pgo-instrumented
```

Exercise that binary through a representative, audited UCI workload while
writing raw profiles without collisions:

```bash
LLVM_PROFILE_FILE="$PWD/testing/private/pgo/raw/%m_%p.profraw" \
FASTCHESS_BIN=/path/to/fastchess \
ENGINE_A=testing/private/neyrang-pgo-instrumented \
ENGINE_B=testing/private/neyrang-pgo-instrumented \
ENGINE_A_NAME=NEYRANG-PGO-train-A ENGINE_B_NAME=NEYRANG-PGO-train-B \
GAMES=256 NODES=50000 CONCURRENCY=8 HASH_MB=64 THREADS=1 \
OPENINGS_FILE=scripts/openings.epd OPENING_ORDER=sequential \
OPENING_SEED=20260916 STRICT=1 \
PGN_OUT=testing/private/pgo/training.pgn \
META_OUT=testing/private/pgo/training.meta.txt \
LOG_OUT=testing/private/pgo/training.log \
CONFIG_OUT=testing/private/pgo/training.config.json \
bash scripts/match.sh
```

Then merge the raw profiles and build the optimized artifact:

```bash
scripts/build-native-pgo.sh optimize testing/private/pgo \
  testing/private/neyrang-pgo
```

The script refuses to overwrite the profile directory, merged profile, or
binary. `llvm-tools-preview` must be installed for the selected Rust toolchain,
or `LLVM_PROFDATA` must name a compatible executable. Before retention, rerun
Perft, deterministic tree/checksum checks, interleaved timing, and a paired
equal-resource match. Never transfer native/PGO measurements to another CPU or
workload without fresh evidence.

## Match testing

Use balanced openings, color-reversed pairs, equal resource limits, frozen
binaries, and a predeclared acceptance rule. Record game count, W/D/L,
pentanomial results, time control, engine options, and abnormal terminations.
`scripts/audit-match.py` accepts UTF-8 logs and BOM-marked UTF-16 logs produced
by Windows PowerShell redirection before checking anomalies and final totals.
Warnings are rejected by default. Compatibility runs may explicitly allow only
the named opponent's well-formed post-threefold PV warning, or its post-threefold
and post-fifty-move PV warnings, with `allow-opponent-threefold-pv` or
`allow-opponent-draw-rule-pv`. Candidate warnings and every other warning remain
fatal to the audit.

Do not treat Perft, tactical positions, node reductions, NPS, or offline model
loss as Elo evidence.

## Private artifacts

### Protocol evidence

Use `scripts.protocol_log.ProtocolLog` for new Python engine-collection logs.
It creates a new file exclusively, without append mode, and compares the final
on-disk bytes with the bytes submitted to the handler. Call `verify()` after
the engine exits and before closing the handler or publishing success. A write
error or hash mismatch invalidates collection even if the engine exited zero.
This storage check does not replace independent raw-UCI, history, or game audits.

Run its regression check with:

```bash
python3 -m unittest scripts.tests.test_protocol_log
```

Generated games, corpora, networks, profiles, machine inventories, absolute
paths, credentials, and host-specific launch files belong under ignored local
directories such as `testing/private/`, never in Git.

The optional policy build is checked with:

```bash
cargo test --locked --all-features
python3 -m unittest scripts.tests.test_fit_shegerd_policy
scripts/test-match-config.sh
```

Policy fitting compares the teacher-selected move only with candidates from
the same runtime MovePicker class (`quiet`, `good tactical`, or `bad tactical`).
Cross-stage accuracy is not evidence because the runtime never lets the policy
move a candidate across those boundaries.

`ENGINE_A_POLICY_FILE` and `ENGINE_B_POLICY_FILE` pass separately hashed
`PolicyFile` artifacts through the paired-match runner. Fast screens use at
most one concurrent game per physical core, one engine thread, paired colors,
ponder off, and fixed nodes. Parallelism may shorten evidence collection but
does not increase either engine's registered resources.

`scripts.teacher_export` creates the complete parent chain for a new shard
output, but still refuses to reuse an existing output directory. Its focused
regression check is:

```bash
python3 -m unittest scripts.tests.test_teacher_export
```

### Teacher corpus preparation

`python3 -m scripts.teacher_corpus` consumes completed `teacher_export` directories
in registered group order. Supply repeated `--input` arguments, the full ordered
`--groups` JSON, a new `--output` directory, and explicit `--count`, `--exposures`
and `--seed` values from the experiment registration.

It verifies export hashes, policy, text/provenance alignment and complete group
coverage before selecting the exact unique-row quota. Selection keeps the first
eligible canonical first-four-field FEN occurrence, then ranks by SHA-256 of
`seed + TAB + group + TAB + decimal ply`, with group/ply tie-breaking. The derived
training text streams repeated copies of the selected base file; repetitions
are exposures, not new positions or independently shuffled epochs. Unique keys
and candidate records remain in memory; this is a bounded-corpus utility, not
an unlimited streaming index.

The manifest binds inputs, output hashes, byte counts and selection settings.
`position-keys.txt` contains all observed TRAIN keys, including ineligible and
capped-game rows, for later evaluation exclusions. The caller must first require
the full campaign's independent source audits, bind the export list and keep
evaluation sets sealed. This command does not launch training or promote an engine.

```bash
python3 -m unittest scripts.tests.test_teacher_corpus
```
