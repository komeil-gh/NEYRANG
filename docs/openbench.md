# OpenBench integration

NEYRANG carries the repository-side contract required for distributed OpenBench testing. This does not mean that a server or public worker pool already exists: deployment still requires a real GitHub remote, a pinned OpenBench fork, an opening-book artifact, server hosting, and trusted workers.

## Repository contract

OpenBench's public-engine requirements call for:

- UCI `Hash` and `Threads` options
- a Makefile that accepts `EXE=<name>`
- the named executable beside that Makefile
- `./<binary> bench` with deterministic nodes and parsable NPS
- the same node result across repeated and concurrent benches

NEYRANG implements that contract at the repository root. The default build uses the `maxperf` Cargo profile and the worker CPU's native target:

```bash
make EXE=NEYRANG-01234567
./NEYRANG-01234567 bench
```

Override `PROFILE` or `TARGET_CPU` only in a registered build configuration:

```bash
make EXE=NEYRANG-01234567 PROFILE=release TARGET_CPU=generic
```

The retained P3 source currently reports 180,591 nodes at the default depth-5 bench. Time and NPS remain host-dependent. Any source change that alters the deterministic tree must update the registered bench only after the change is understood and accepted.

OpenBench uses the configured `rustc>=1.98` entry to decide whether a worker can build NEYRANG, then invokes `make -j EXE=...` without a C/C++ compiler override for Rust builds. Every registered worker must therefore provide both Rust 1.98 or newer and Cargo; compiler discovery alone does not install the Rust toolchain.

Verify the complete local contract with:

```bash
scripts/test-openbench-contract.sh
```

The test builds a uniquely named binary, runs three sequential and three concurrent benches, requires the registered node count and positive NPS, verifies `Hash`, `Threads`, `uciok`, and `readyok`, and removes all temporary artifacts on exit.

## Server-side engine configuration

The following is a starting template for `Engines/NEYRANG.json` in a separate pinned OpenBench fork. Replace the repository URL and measured reference NPS; do not publish placeholders as a live configuration.

```json
{
  "private": false,
  "nps": 2000000,
  "source": "https://github.com/<owner>/<neyrang-repository>",
  "build": {
    "path": ".",
    "compilers": ["rustc>=1.98"],
    "cpuflags": [],
    "systems": ["Darwin", "Linux"]
  },
  "test_presets": {
    "default": {
      "base_branch": "main",
      "book_name": "<audited-balanced-book>.epd",
      "test_bounds": "[0.00, 3.00]",
      "test_confidence": "[0.05, 0.05]",
      "win_adj": "movecount=3 score=400",
      "draw_adj": "movenumber=40 movecount=8 score=10"
    },
    "STC": {
      "both_options": "Threads=1 Hash=16 Move Overhead=100",
      "both_time_control": "8.0+0.08",
      "workload_size": 32
    },
    "LTC": {
      "both_options": "Threads=1 Hash=128 Move Overhead=100",
      "both_time_control": "60.0+0.60",
      "workload_size": 8
    }
  }
}
```

The example bounds and time controls are defaults to review, not permission to reuse them for every experiment. Each test must freeze its own hypothesis, base/dev SHAs, bench values, options, book SHA, pairing, stopping rule, adjudication, worker eligibility, and result-retention policy.

## Deployment gates

Do not accept public workloads until all of the following are true:

1. Select and record the project license. The current repository has no license.
2. Create the real GitHub remote and fast-forward the intended default branch without rewriting or backdating history.
3. Publish immutable release/source refs and confirm that clean Linux and Darwin workers with Rust 1.98+ and Cargo can build them with `make EXE=...`.
4. Fork OpenBench, pin the client/server ref, configure NEYRANG and the audited book, enable authentication, and keep secrets outside Git.
5. Measure the reference NPS on the chosen baseline rather than copying the placeholder above.
6. Run a small fixed-game smoke and independently audit every PGN/log/build identity.
7. Run one known no-op or same-source workload to verify deterministic build and pair accounting.
8. Only then open a preregistered SPRT to additional workers.

The project's existing fixed-shard tool remains useful for frozen fixed-size campaigns and independent replay. It is not a substitute for OpenBench's sequential global stopping logic.

## Data generation boundary

OpenBench also supports distributed data generation through a `genfens` command that emits seeded opening FENs. NEYRANG does not implement that command yet. It must be designed with exact 64-bit seed use, deterministic diversity tests, stall handling, and a versioned game/binpack schema before NNUE data generation is registered.

## Primary references

- [OpenBench public-engine requirements](https://github.com/AndyGrant/OpenBench/wiki/Requirements-For-Public-Engines)
- [OpenBench bench parser](https://github.com/AndyGrant/OpenBench/blob/master/Client/bench.py)
- [SPRT and fixed-game workloads](https://github.com/AndyGrant/OpenBench/wiki/SPRT-and-Fixed%E2%80%90Game-Workloads)
- [Data-generation workloads and `genfens`](https://github.com/AndyGrant/OpenBench/wiki/Data-Generation-Workloads)
- [OpenBench engine configuration](https://github.com/AndyGrant/OpenBench/wiki/Configuring-Your-OpenBench-Instance)
