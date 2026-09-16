# SANJ

SANJ is the judgment NEYRANG evaluates with. The normal development build uses
the official Stockfish 18 small NNUE network through a compatible CPU evaluator.
The deterministic tapered handcrafted evaluator remains the explicit
`EvalFile=<empty>` fallback. It calculates separate middlegame and endgame
scores, derives phase from remaining non-pawn material, interpolates, then
returns a side-to-move score with a 6-centipawn tempo term.

Implemented terms:

- material
- original formula-based piece-square values
- bishop pair
- legal-geometry mobility approximation
- isolated, doubled, and passed pawns
- rook open and semi-open files
- king pawn shield
- bounded nonlinear king-ring pressure from coordinated attackers
- tempo

The compact formula-based PSQT is intentionally inspectable and avoids importing unexplained tables. All terms are symmetric under color/rank mirroring. Tests confirm symmetric positions and material sign.

King danger uses a deliberately bounded coordination gate: a lone minor-piece gesture scores nothing, while multiple attackers—or one attacker backed by a queen—combine attacked king-zone squares and attacker weights into a middlegame penalty capped at 120 centipawns. It is a compact SANJ-specific model, not a copied evaluation table.

H9 computes each non-pawn attack bitboard once per classical evaluation and
reuses it for both mobility and king pressure. The score formula and every
weight remain unchanged; the exact 100,000-position SANJ trace is the binding
semantic oracle.

Known limitations include no pawn hash, no general threats/space/outposts, and no systematic tuning against game data. The evaluator remains an inspectable bootstrap rather than a claim that handcrafted weights are finished.

## NNUE judgment

The default `stockfish-nnue` feature ships the accepted external-network path:

```bash
cargo build --release
target/release/neyrang bench-nnue /path/to/network.nnue 5
```

The UCI string option `EvalFile` accepts either a complete supported
`NEYRANG\0` artifact or a Stockfish-compatible HalfKAv2 network. Version 1 of
the native format uses Chess768 and version 2 uses the registered three-bank
horizontally mirrored Chess768x3hm mapping.
Version 3 keeps that input mapping and adds four material-routed output heads;
H5f's independent heads and the trainer-only H5g shared-weight/phase-bias
follow-up were both rejected at their offline statistical gates. Version 3
therefore remains a supported experimental artifact contract without a
retained network.
Version 4 registers the rejected H5h `Chess768x3hmli` experiment: the same incremental
transformer feeds one 384-value head comprising both SCReLU perspectives and
their elementwise absolute difference. It adds no hidden layer or accumulator
state. It improved fresh teacher MSE by 1.51%, but its paired-bootstrap interval
crossed zero; a later 256-game fixed-node check scored 48.44% against N7, so it
remains rejected.
`EvalMix` selects the NNUE percentage from 0 through 100 while retaining
classical SANJ for the remainder; its default is 100 for the accepted embedded
network. The Stockfish-compatible evaluator currently performs a full refresh
per static evaluation; incremental updates are a future throughput optimization,
not part of the accepted H27 strength change.
Decoding is fail-closed. The loaded network is immutable and shared across
Lazy-SMP workers, while every worker owns a move-delta accumulator stack. The
engine scalar output is checked bit-for-bit against the independent reference
crate on frozen FEN suites.

The native N7 residual remains supported for controlled experiments, not as the
default or an absolute Elo claim.
The retained N1e-16M artifact passed bit-exact inference and deterministic
benchmark gates but scored only 27.20% in its independently audited 1,000-game
equal-node screen (`177/633/190`). That network is rejected and must not be made
the default. A replacement still requires a new untouched final holdout,
fixed-node and equal-time games, a normalized SPRT, longer-time-control
confirmation, and an independent artifact audit. `EvalFile=<empty>` selects
classical SANJ, while `<embedded>` restores the Stockfish-18 small network.

The embedded file is `nn-37f18f62d772.nnue`, SHA-256
`37f18f62d772f3107e1d6aaca3898c130c3c86f2ab63e6555fbbca20635a899d`.
It is an official Stockfish 18 asset, not a network trained by NEYRANG; see the
repository's third-party notices for source and licensing.

The later N2d version-2 candidate passed parity, fresh selection and an
untouched holdout, but scored `343/356/301` (`49.35%`) in its clean 1,000-game
fixed-node screen against classical SANJ. Its registered floor was 50%, so it is
also rejected and received zero equal-time or SPRT games. These runner results
are selection evidence, not a release Elo estimate.

## Exact SANJ trace

The development branch includes a feature-gated, streaming trace for dataset work. It is an independent reconstruction rather than instrumentation in the tournament hot path. The normal release remains byte-for-byte identical to the accepted G1 binary.

Build and run it explicitly:

```bash
cargo build --release --features sanj-tools
target/release/neyrang sanj-trace positions.tsv > features.tsv
```

Use `-` instead of a path to read stdin. Each non-comment input row is exactly:

```text
record_id<TAB>target<TAB>FEN
```

`target` is White-relative WDL space: `0` is a Black win, `0.5` is a draw, and `1` is a White win. Soft labels within `[0,1]` are accepted, but their provenance must be recorded. Output schema `neyrang-sanj-trace-v2` canonicalizes the FEN and writes fixed-order White-minus-Black coefficients for:

- piece counts and all ten raw PSQT rank/edge/center bases
- bishop pair
- doubled-extra, isolated, and squared passed-pawn advancement
- knight, bishop, rook, and queen mobility
- open and semi-open rook files
- king-shield pawns and bounded coordinated king danger
- phase plus exact middlegame, endgame, White-relative, tempo, and side-to-move scores

The raw coefficients reconstruct every current constant and formula exactly. A deterministic 100,000-position oracle compares the reconstruction with production evaluation. The schema is accounting infrastructure, not permission to change weights or add terms.

The incremental interface exists behind the default-enabled `nnue` feature. Its
accumulator also carries the exact remaining-piece count needed by version 3's
constant-time output-head selection. Version 4 derives its extra channel
directly from those same two accumulators. Retained N7 activation uses a
runtime-checked AVX2 dot product on supported x86-64 processors and the
bit-exact scalar path everywhere else; version 4 also remains scalar. A future
NEON path must preserve the same oracle. Training tooling remains separate from
the engine build.
