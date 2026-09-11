# NEYRANG · نیرنگ

An independent UCI chess engine written from scratch in Rust, with its own
chess rules, search, and evaluation. The playing core uses only the Rust
standard library. NEYRANG runs inside a UCI-compatible chess interface or
directly from the terminal.

[Build](#build-and-run) · [UCI](#using-a-chess-interface) ·
[Development](#development) · [Documentation](#documentation) ·
[تقدیم‌نامه](#dedication)

<a id="dedication"></a>
<div dir="rtl" lang="fa" align="right">

<h2>به رسم سپاس</h2>

<blockquote>
<p>
مردمانِ بخرد اندر هر زمان<br>
رازِ دانش را به‌هر گونه زبان<br>
گِرد کردند و گرامی داشتند<br>
تا به سنگ اندر همی بنگاشتند
</p>
<p>— <a href="https://ganjoor.net/roodaki/masnaviha/kalila-sand/sh13">رودکی</a></p>
</blockquote>

<p>آنچه امروز از شطرنج ایران به ما رسیده، میراث رنج و همت کسانی است که پیش از ما راه گشودند، آموختند و دانش خویش را به دیگری سپردند.</p>

<p>از این رو <strong>نیرنگ</strong> را، به رسم سپاس و حق‌شناسی، به یاد و حرمت <strong>عبدالحسین نوابی، یوسف صفوت، خسرو شیخ هرندی، عباس لطفی و سید محمدکاظم مرتضوی</strong>، و به همهٔ آنان تقدیم می‌کنم که چیزی بر میراث شطرنج ایران افزودند.</p>

<p>و در زندگی خود، به <strong>پدربزرگم که نخستین بار مرا با شطرنج آشنا کرد</strong>؛ و به همهٔ کسانی که پس از او، دانسته‌ای به من آموختند، اندیشه‌ای در من برانگیختند یا چیزی از این بازی را با من قسمت کردند. اگر امروز من نیز چیزی بر این راه می‌افزایم، سهمی از آن از ایشان است.</p>

<p>
با احترام،<br>
<strong>کمیل</strong><br>
<em>عضوی کوچک از جامعهٔ شطرنج ایران</em><br>
طهران — ۱۴۰۳ هجری خورشیدی
</p>

</div>

## Versions

| Branch | Engine version | Scope |
| --- | --- | --- |
| [`main`](https://github.com/komeil-gh/NEYRANG/tree/main) | `0.2.0` | Stable baseline; single-threaded search and classical evaluation |
| [`dev/0.3-search`](https://github.com/komeil-gh/NEYRANG/tree/dev/0.3-search) | `0.3.0-dev` | Search and timing improvements, parallel search, and experimental evaluation tooling |

The development branch is unreleased. Classical evaluation remains the default;
experimental NNUE requires a separate feature build and network file. No NNUE
network is bundled or recommended for play.

## Build and run

You need Git, Rust **1.98 or newer**, Cargo, and a native linker. The repository
selects the stable Rust toolchain and uses Edition 2024. On macOS, install
Apple's command-line tools with `xcode-select --install` if they are missing.

```bash
git clone --branch main https://github.com/komeil-gh/NEYRANG.git
cd NEYRANG
cargo build --release --locked
```

The executable is `target/release/neyrang` on macOS/Linux and
`target/release/neyrang.exe` on Windows. Run a deterministic benchmark:

```bash
target/release/neyrang bench
```

To build the development version from the same clone:

```bash
git switch dev/0.3-search
cargo build --release --locked
```

## Using a chess interface

Add the executable as a **UCI engine** in your chess interface. NEYRANG provides
the engine; the interface provides the board, clocks, and game controls.
The [En Croissant guide](docs/en-croissant.md) covers engine registration and
versioned local builds on macOS.

| UCI option | `0.2.0` | `0.3.0-dev` |
| --- | --- | --- |
| `Hash` | 64 MB by default | 64 MB by default |
| `Threads` | Accepted; search uses one thread | Defaults to 1; higher values enable Lazy SMP |
| `Move Overhead` | 10 ms by default | 30 ms by default |
| `EvalFile` | Not available | Available with the `nnue` feature; empty selects classical evaluation |

For a protocol check in a POSIX shell:

```bash
printf 'uci\nisready\nquit\n' | target/release/neyrang
```

The response includes the engine identity, available options, `uciok`, and
`readyok`. Run the executable without arguments for an interactive UCI session.

## Inside the engine

- **Chess rules:** bitboards, FEN parsing, full legal move generation, reversible
  make/unmake, Zobrist hashing, and repetition and draw detection.
- **Search:** iterative deepening, alpha-beta/PVS, quiescence, aspiration
  windows, transposition tables, SEE-based move ordering, and late move reductions.
- **Evaluation:** a handcrafted tapered evaluator. The development branch also
  provides opt-in NNUE inference and separate data and training tools.
- **Verification:** Perft fixtures, tactical and protocol tests, deterministic
  benchmarks, paired engine matches, and sequential probability ratio tests (SPRT).

On the development branch, **REKHNE** owns search, **SANJ** owns evaluation,
and **SHEGERD** owns move ordering. Their boundaries are documented in the
[naming guide](https://github.com/komeil-gh/NEYRANG/blob/dev/0.3-search/docs/naming.md).

## Verification and benchmarks

```bash
target/release/neyrang perft 5
target/release/neyrang divide 4
target/release/neyrang bench
```

Recorded Perft reference results:

| Position | Depth | Nodes |
| --- | ---: | ---: |
| Initial position | 5 | 4,865,609 |
| Canonical Kiwipete | 4 | 4,085,603 |
| Rook/pawn endgame | 5 | 674,624 |

Exact FENs and test commands are in the [testing guide](docs/testing.md).
The `0.2.0` depth-5 benchmark visits **196,627 nodes** with checksum
`4a4c31e290740db3`. Node counts and checksums identify a particular version and
workload; elapsed time and nodes per second depend on the machine and build.

Playing-strength results are reported with their opponents, time controls,
sample sizes, and decision rules in the
[experiment ledger](https://github.com/komeil-gh/NEYRANG/blob/dev/0.3-search/docs/development/experiments.md).
Perft, benchmark speed, and offline training results are not Elo ratings.

## Development

Run the core checks from either branch:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --locked --all-features
scripts/test-match-config.sh
```

The development branch adds Python infrastructure checks, independent NNUE
reference tests, and the OpenBench build contract. Follow the
[contribution guide](https://github.com/komeil-gh/NEYRANG/blob/dev/0.3-search/CONTRIBUTING.md)
for prerequisites and the full set of checks. Search and evaluation experiments
record their hypothesis, baseline, test conditions, and acceptance rule before
collecting results.

Experimental NNUE, on `dev/0.3-search` only:

```bash
cargo build --release --locked --features nnue
```

This build does not embed a network. Loading a compatible, separately validated
network requires the UCI `EvalFile` option. The
[NNUE reference specification](https://github.com/komeil-gh/NEYRANG/blob/dev/0.3-search/docs/development/nnue-reference.md)
and [data and training guide](https://github.com/komeil-gh/NEYRANG/blob/dev/0.3-search/docs/development/nnue-data.md)
describe formats, reproducibility requirements, and experimental results.

## Documentation

The local [architecture](docs/architecture.md), [testing](docs/testing.md), and
[changelog](CHANGELOG.md) describe the checked-out branch. The following links
open the development documentation:

- **Engine:** [REKHNE search](https://github.com/komeil-gh/NEYRANG/blob/dev/0.3-search/docs/rekhne.md)
  and [SANJ evaluation](https://github.com/komeil-gh/NEYRANG/blob/dev/0.3-search/docs/sanj.md).
- **Integration:** [OpenBench](https://github.com/komeil-gh/NEYRANG/blob/dev/0.3-search/docs/openbench.md)
  and [search profiling](https://github.com/komeil-gh/NEYRANG/blob/dev/0.3-search/docs/development/search-observability.md).
- **Research:** [development roadmap](https://github.com/komeil-gh/NEYRANG/blob/dev/0.3-search/docs/development/competitive-roadmap.md)
  and [experiment records](https://github.com/komeil-gh/NEYRANG/blob/dev/0.3-search/docs/development/experiments.md).

## Contributing, security, and license

Read [CONTRIBUTING.md](https://github.com/komeil-gh/NEYRANG/blob/dev/0.3-search/CONTRIBUTING.md)
before proposing a change. Use the process in
[SECURITY.md](https://github.com/komeil-gh/NEYRANG/blob/dev/0.3-search/SECURITY.md)
for security-sensitive reports.

No project license has been selected yet. The repository is publicly readable;
an open-source license has not been granted.
