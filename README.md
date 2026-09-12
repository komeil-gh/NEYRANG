# NEYRANG · نیرنگ

NEYRANG is an independent chess engine for play, analysis, and reproducible
engine research. Written from scratch in Rust, it implements its own chess
rules, search, and evaluation using only the standard library in the playing
core. It speaks the Universal Chess Interface (UCI) protocol and includes
command-line tools for testing and benchmarking.

[Build](#build-and-run) · [Play and analyze](#play-and-analyze) ·
[Verification](#verification-and-benchmarks) · [Documentation](#documentation) ·
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

<p>
از این رو <strong>نیرنگ</strong> را، به رسم سپاس و حق‌شناسی، به یاد و حرمت
<strong><a href="https://www.fide.com/images/stories/NEWS_2011/fide_news/Agenda_and_Annexes_2011/Minutes-Krakow-draft2.pdf#page=1" title="یادبود عبدالحسین نوابی در صورت‌جلسهٔ فیده، ۲۰۱۱ — PDF، صفحهٔ ۱">عبدالحسین نوابی</a></strong>،
<strong><a href="https://www.olimpbase.org/players/2cmci8ye.html" title="سوابق یوسف صفوت در المپیادهای شطرنج — OlimpBase">یوسف صفوت</a></strong>،
<strong><a href="https://old.fide.com/component/content/article/15-chess-news/11359-obituary-im-khosro-sheikh-harandi.html" title="یادنامهٔ خسرو شیخ هرندی در وبگاه فیده">خسرو شیخ هرندی</a></strong>،
<strong><a href="https://www.olimpbase.org/players/v557p13c.html" title="سوابق عباس لطفی در المپیادهای شطرنج — OlimpBase">عباس لطفی</a></strong> و
<strong><a href="https://ratings.fide.com/profile/12500046" title="پروفایل رسمی کاظم مرتضوی در فیده">سید محمدکاظم مرتضوی</a></strong>،
و به همهٔ آنان تقدیم می‌کنم که چیزی بر میراث شطرنج ایران افزودند.
</p>

<p>و در زندگی خود، به <strong>پدربزرگم که نخستین بار مرا با شطرنج آشنا کرد</strong>؛ و به همهٔ کسانی که پس از او، دانسته‌ای به من آموختند، اندیشه‌ای در من برانگیختند یا چیزی از این بازی را با من قسمت کردند. اگر امروز من نیز چیزی بر این راه می‌افزایم، سهمی از آن از ایشان است.</p>

<p>
با احترام،<br>
<strong>کمیل</strong><br>
<em>عضوی کوچک از جامعهٔ شطرنج ایران</em><br>
طهران — ۱۴۰۳ هجری خورشیدی
</p>

</div>

## Choose a version

| Branch | Engine version | Scope |
| --- | --- | --- |
| [`main`](https://github.com/komeil-gh/NEYRANG/tree/main) | `0.2.0` | Stable baseline; single-threaded search and classical evaluation |
| [`dev/0.3-search`](https://github.com/komeil-gh/NEYRANG/tree/dev/0.3-search) | `0.3.0-dev` | Search and timing improvements, parallel search, and experimental evaluation tooling |

Use `main` for the `0.2.0` baseline and `dev/0.3-search` to work with the newer
engine and research tools. Development results do not constitute a `0.3.0`
release. Both branches use classical evaluation by default.

## Build and run

**Requirements:** Git, Rust **1.98 or newer** with Cargo, and a native linker.
The repository selects the stable Rust toolchain and uses Edition 2024. On
macOS, `xcode-select --install` installs the required command-line tools if
they are missing.

```bash
git clone --branch main https://github.com/komeil-gh/NEYRANG.git
cd NEYRANG
cargo build --release --locked
```

Run a benchmark to check the build:

```bash
target/release/neyrang bench
```

On Windows, the executable is `target/release/neyrang.exe`; in PowerShell, run
`.\target\release\neyrang.exe bench`. The remaining shell examples use POSIX
syntax.

To build the development version from the same clone:

```bash
git switch dev/0.3-search
cargo build --release --locked
```

## Play and analyze

Add `target/release/neyrang` (or `neyrang.exe` on Windows) as a **UCI engine**
in your chess interface. Use the interface to play a game or analyze a position;
NEYRANG itself does not include a graphical board.
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
`readyok`.

To analyze from the terminal, run the executable without arguments and enter:

```text
uci
isready
position startpos moves e2e4 e7e5
go depth 8
```

The engine prints search information followed by `bestmove`. Enter `stop` to
interrupt a search, or `quit` to close the session.

## Design

- **Chess rules:** bitboards, FEN parsing, full legal move generation, reversible
  make/unmake, Zobrist hashing, and repetition and draw detection.
- **Search:** iterative deepening, alpha-beta/PVS, quiescence, aspiration
  windows, transposition tables, SEE-based move ordering, and late move reductions.
- **Evaluation:** a handcrafted tapered evaluator. The development branch also
  provides opt-in NNUE inference and separate data and training tools.
- **Time and protocol:** asynchronous UCI commands, cooperative search stopping,
  and soft/hard time limits. The development branch adds Lazy SMP parallel search.

On the development branch, **REKHNE** owns search, **SANJ** owns evaluation,
and **SHEGERD** owns move ordering. Chess rules remain separate from these
components. The boundaries are documented in the
[naming guide](https://github.com/komeil-gh/NEYRANG/blob/dev/0.3-search/docs/naming.md).

## Verification and benchmarks

```bash
target/release/neyrang perft 5
target/release/neyrang divide 4
```

Perft counts legal move sequences; `divide` reports the count for each root
move. Recorded reference results:

| Position | Depth | Nodes |
| --- | ---: | ---: |
| Initial position | 5 | 4,865,609 |
| Canonical Kiwipete | 4 | 4,085,603 |
| Rook/pawn endgame | 5 | 674,624 |

Exact FENs and test commands are in the [testing guide](docs/testing.md).
The `0.2.0` depth-5 benchmark visits **196,627 nodes** with checksum
`4a4c31e290740db3`. Node counts and checksums identify a particular version and
workload; elapsed time and nodes per second depend on the machine and build.

Search changes are checked with tactical tests, deterministic benchmarks,
color-reversed engine matches, and sequential probability ratio tests (SPRT).
Playing-strength results are recorded with their opponents, time controls,
sample sizes, and decision rules in the
[experiment ledger](https://github.com/komeil-gh/NEYRANG/blob/dev/0.3-search/docs/development/experiments.md).
Perft, benchmark speed, and offline training results are not Elo ratings.

## Development

Run the core checks from a POSIX shell on either branch:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --locked --all-features
scripts/test-match-config.sh
```

The development branch also requires Python infrastructure tests, independent
NNUE reference tests, and the OpenBench build contract. The
[contribution guide](https://github.com/komeil-gh/NEYRANG/blob/dev/0.3-search/CONTRIBUTING.md)
lists their prerequisites and commands.

Keep each search or evaluation experiment focused. Record the hypothesis,
baseline, test conditions, and acceptance rule before collecting results;
retain or reject the change against that rule. Store detailed results in the
experiment ledger so they can be reviewed alongside the code.

### Experimental NNUE

Available on `dev/0.3-search` only:

```bash
cargo build --release --locked --features nnue
```

This build does not embed a network. Set the UCI `EvalFile` option to a
compatible, separately validated network; leave it empty for classical
evaluation. No NNUE network is bundled or recommended for play. Format and
training documentation is linked below.

## Documentation

Start with the [architecture](docs/architecture.md), [testing guide](docs/testing.md),
or [changelog](CHANGELOG.md) for the checked-out branch. Detailed development
guides are grouped by task:

| Task | Development documentation |
| --- | --- |
| Understand search and evaluation | [REKHNE](https://github.com/komeil-gh/NEYRANG/blob/dev/0.3-search/docs/rekhne.md) · [SANJ](https://github.com/komeil-gh/NEYRANG/blob/dev/0.3-search/docs/sanj.md) |
| Integrate or profile the engine | [OpenBench](https://github.com/komeil-gh/NEYRANG/blob/dev/0.3-search/docs/openbench.md) · [Search profiling](https://github.com/komeil-gh/NEYRANG/blob/dev/0.3-search/docs/development/search-observability.md) |
| Plan and assess experiments | [Roadmap](https://github.com/komeil-gh/NEYRANG/blob/dev/0.3-search/docs/development/competitive-roadmap.md) · [Experiment ledger](https://github.com/komeil-gh/NEYRANG/blob/dev/0.3-search/docs/development/experiments.md) |
| Work on NNUE | [Reference implementation](https://github.com/komeil-gh/NEYRANG/blob/dev/0.3-search/docs/development/nnue-reference.md) · [Data and training](https://github.com/komeil-gh/NEYRANG/blob/dev/0.3-search/docs/development/nnue-data.md) |

## Contributing and security

Bug reports should include the branch and commit, build command, input position
or UCI commands, and expected and observed behavior. Follow
[CONTRIBUTING.md](https://github.com/komeil-gh/NEYRANG/blob/dev/0.3-search/CONTRIBUTING.md)
for changes and experiment proposals. Use the process in
[SECURITY.md](https://github.com/komeil-gh/NEYRANG/blob/dev/0.3-search/SECURITY.md)
for security-sensitive reports.

## License

NEYRANG is free software licensed under the
[GNU General Public License v3.0 or later](LICENSE) (`GPL-3.0-or-later`).
See [AUTHORS](AUTHORS) for project authorship.

No NNUE network or training dataset is bundled with the engine. Experimental
networks and corpora mentioned in the development records are separate local
artifacts, not release assets; any future distributed artifact must state its
own license and data provenance.
