# Contributing to NEYRANG

NEYRANG is developed by measured experiments. Correctness is mandatory; a search or evaluation idea stays only after it passes its preregistered deterministic and game gates.

## Before changing code

Build the current branch and record its identity:

```bash
git rev-parse HEAD
cargo build --release --locked
shasum -a 256 target/release/neyrang
target/release/neyrang bench
```

For a playing change, open a **Strength experiment** issue before collecting decision evidence. Freeze the parent source/binary, hypothesis, exact scope, benchmark expectations, opening set, resource controls, stopping rule, and reversion condition.

Do not copy another engine's code or tuned constants. Primary sources may justify an experiment, but NEYRANG must derive its own implementation and earn its own parameters. Record the provenance and license of every external dataset, network, book, or tool.

## Code boundaries

- `src/chess`: rules, representation, move generation, make/unmake, hashing, Perft
- `src/rekhne`: REKHNE search strategy, TT, time, limits, and parallel control
- `src/sanj`: SANJ search-independent position judgment
- `src/shegerd`: SHEGERD move ordering, history heuristics, and SEE
- `src/uci`: protocol parsing and engine boundary
- `src/tools` and `scripts`: deterministic developer and evidence tooling

Keep the runtime dependency surface minimal. The default playing binary permits
the documented `nnue-rs` dependency for Stockfish-compatible NNUE inference;
any additional dependency requires an explicit proposal. Preserve
scalar/reference implementations when adding platform-specific acceleration.

## Required local gates

```bash
make quality
python3 -m venv .venv
.venv/bin/pip install -r scripts/requirements-sanj.txt
.venv/bin/python -m unittest discover -s scripts/tests
scripts/test-match-config.sh
cargo build --release --locked
scripts/test-openbench-contract.sh
```

Run the three explicit Perft commands in [the testing guide](docs/testing.md#perft). A deterministic tree/checksum change must be explained; it is never dismissed as a speed optimization.

## Playing-strength evidence

Use color-reversed pairs, an audited balanced opening set, equal resources, frozen binaries, full PGN telemetry, and an independent result audit. Report game count, W/D/L, pentanomial counts, score, confidence interval or SPRT decision, and every abnormal termination. Fixed-node results isolate decision quality but do not replace equal-time testing when NPS changes.

Small samples, Perft, puzzle suites, node reductions, NPS, or offline loss are not Elo evidence. Revert a candidate that fails its registered rule instead of preserving it for narrative value.

## Pull requests

Keep commits scoped and use the pull-request evidence template. Documentation-only and infrastructure changes should say why they cannot change playing semantics. Generated bulk games, corpora, binaries, networks, local paths, credentials, and machine-specific build products do not belong in Git.

## License boundary

Contributions are accepted under the project's
[`GPL-3.0-or-later`](LICENSE) license. By submitting a contribution, you confirm
that you have the right to provide it under those terms. Preserve copyright and
license notices on code or data that may legally be redistributed, and keep
externally licensed networks, datasets, books, and generated artifacts out of
the repository unless their provenance and compatible distribution terms are
documented.
