# Testing NEYRANG

This guide covers the public, reproducible checks for the engine. Keep private
datasets, machine-specific paths, host details, and raw experiment output out
of the repository.

## Core checks

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --locked --all-features
cargo build --release --locked
scripts/test-match-config.sh
```

## Perft

```bash
target/release/neyrang perft 5
target/release/neyrang perft-fen \
  "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1" 4
target/release/neyrang perft-fen \
  "8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1" 5
```

Expected node counts are 4,865,609; 4,085,603; and 674,624 respectively.

## UCI smoke test

```bash
printf 'uci\nisready\nposition startpos\ngo depth 4\nquit\n' \
  | target/release/neyrang
```

A successful run prints `uciok`, `readyok`, search information, and a legal
`bestmove`.

## Benchmark

```bash
target/release/neyrang bench
```

For NEYRANG 0.2.0, the fixed depth-5 workload visits 196,627 nodes with
checksum `4a4c31e290740db3`. Timing and nodes per second depend on the machine,
operating system, compiler, and build flags.

## Match testing

Use balanced openings, color-reversed pairs, equal resource limits, frozen
binaries, and a predeclared acceptance rule. Record game count, W/D/L,
pentanomial results, confidence bounds, engine identities, and all abnormal
terminations. The bundled openings are suitable for smoke tests, not strength
claims.
