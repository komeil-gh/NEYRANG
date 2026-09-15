# SANJ

SANJ is the judgment NEYRANG evaluates with. It currently uses a deterministic
tapered handcrafted evaluation. It calculates separate middlegame and endgame
scores, derives phase from remaining non-pawn material, interpolates, then
returns a side-to-move score with a 12-centipawn tempo term.

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

Known limitations include no pawn hash, no general threats/space/outposts, and no systematic tuning against game data. The evaluator remains an inspectable bootstrap rather than a claim that handcrafted weights are finished.

## Experimental NNUE judgment

The non-default `nnue` feature adds the first N2 engine path without changing
classical SANJ as the default:

```bash
cargo build --release --features nnue
target/release/neyrang bench-nnue /path/to/network.nnue 5
```

The feature exposes the UCI string option `EvalFile`. A non-empty path must be a
complete supported `NEYRANG\0` artifact: version 1 uses Chess768 and version 2
uses the registered three-bank horizontally mirrored Chess768x3hm mapping.
`EvalMix` selects the NNUE percentage from 0 through 100 while retaining
classical SANJ for the remainder; its default is 100 for compatibility with
the existing pure-network evidence.
Decoding is fail-closed. The loaded network is immutable and shared across
Lazy-SMP workers, while every worker owns a move-delta accumulator stack. The
engine scalar output is checked bit-for-bit against the independent reference
crate on frozen FEN suites.

This is an experimental playing path, not a retained network or an Elo claim.
The retained N1e-16M artifact passed bit-exact inference and deterministic
benchmark gates but scored only 27.20% in its independently audited 1,000-game
equal-node screen (`177/633/190`). That network is rejected and must not be made
the default. A replacement still requires a new untouched final holdout,
fixed-node and equal-time games, a normalized SPRT, longer-time-control
confirmation, and an independent artifact audit. An empty `EvalFile` value
selects classical SANJ.

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

`target` is White-relative WDL space: `0` is a Black win, `0.5` is a draw, and `1` is a White win. Soft labels within `[0,1]` are accepted, but their provenance must be recorded. Output schema `neyrang-sanj-trace-v3` canonicalizes the FEN and writes fixed-order White-minus-Black coefficients for:

- piece counts and all ten raw PSQT rank/edge/center bases
- bishop pair
- doubled-extra, isolated, and squared passed-pawn advancement
- knight, bishop, rook, and queen mobility
- open and semi-open rook files
- king-shield pawns and bounded coordinated king danger
- bounded direct and safe latent pawn leverage
- phase plus exact middlegame, endgame, White-relative, tempo, and side-to-move scores

The raw coefficients reconstruct every current constant and formula exactly. A deterministic 100,000-position oracle compares the reconstruction with production evaluation. The schema is accounting infrastructure, not permission to change weights or add terms.

The scalar incremental interface now exists behind the non-default `nnue`
feature. NEON/SIMD remains future work and must stay bit-exact with the scalar
oracle. Training tooling remains separate from the engine build.
