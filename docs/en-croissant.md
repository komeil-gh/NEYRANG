# En Croissant integration

NEYRANG is a native UCI executable, so En Croissant can load it as a local engine without an adapter. The helper below builds a release binary and copies it to a stable, versioned path that is safe to keep in En Croissant's engine list:

```bash
scripts/en-croissant.sh prepare
```

The command prints the exact executable path, for example:

```text
.../dist/en-croissant/neyrang-0.3.0-dev-macos-arm64
```

In En Croissant, open **Engines**, choose the local-engine add action, and select
that executable. En Croissant probes it with `uci` and `isready`; the detected
development identity should be `NEYRANG 0.3.0-dev`, with `Hash`, `Threads`, and
`Move Overhead` settings.

The prepared artifact is separate from `target/`, so cleaning Cargo build output does not leave the GUI pointing at a missing file. Run `prepare` again after changing the engine. The generated `dist/` directory is intentionally not committed.

## Viewing games

The repository preserves the pre-rename paired smoke-match PGN at
`examples/legacy/neyrang-vs-stockfish-smoke.pgn`. Its headers remain historical.
Open it directly from En Croissant or run:

```bash
scripts/en-croissant.sh open
```

You can also open any other PGN:

```bash
scripts/en-croissant.sh open /absolute/path/to/games.pgn
```

## Regenerating the smoke PGN

PGN generation is developer tooling and does not add a runtime dependency to NEYRANG. Use an isolated Python environment with the pinned harness dependency:

```bash
python3 -m venv .venv
.venv/bin/pip install -r scripts/requirements.txt
cargo build --release
.venv/bin/python scripts/generate-smoke-pgn.py \
  --neyrang target/release/neyrang \
  --stockfish /absolute/path/to/stockfish \
  --output results/neyrang-vs-stockfish-smoke.pgn
```

The harness runs the first five EPD openings twice with colors reversed. By
default NEYRANG searches to depth 3 and Stockfish to depth 1. These deliberately
unequal settings exercise UCI and legal complete-game behavior; they are not a
strength comparison or Elo evidence. Publish a current NEYRANG PGN only after
running this harness with exact engine identities; never rewrite the legacy PGN.
