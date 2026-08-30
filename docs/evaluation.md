# Evaluation

NEYRANG currently uses a deterministic tapered handcrafted evaluation. It calculates separate middlegame and endgame scores, derives phase from remaining non-pawn material, interpolates, then returns a side-to-move score with a 12-centipawn tempo term.

Implemented terms:

- material
- original formula-based piece-square values
- bishop pair
- legal-geometry mobility approximation
- isolated, doubled, and passed pawns
- rook open and semi-open files
- basic king pawn shield
- tempo

The compact formula-based PSQT is intentionally inspectable and avoids importing unexplained tables. All terms are symmetric under color/rank mirroring. Tests confirm symmetric positions and material sign.

Known limitations include limited king attack modeling, no pawn hash, no threats/space/outposts, and no tuning against game data. The evaluator is a clean bootstrap for search, not a strength claim.

## Exact evaluation trace

The development branch includes a feature-gated, streaming trace for dataset work. It is an independent reconstruction rather than instrumentation in the tournament hot path. The normal release remains byte-for-byte identical to the accepted G1 binary.

Build and run it explicitly:

```bash
cargo build --release --features eval-tools
target/release/neyrang eval-trace positions.tsv > features.tsv
```

Use `-` instead of a path to read stdin. Each non-comment input row is exactly:

```text
record_id<TAB>target<TAB>FEN
```

`target` is White-relative WDL space: `0` is a Black win, `0.5` is a draw, and `1` is a White win. Soft labels within `[0,1]` are accepted, but their provenance must be recorded. Output schema `neyrang-eval-trace-v1` canonicalizes the FEN and writes fixed-order White-minus-Black coefficients for:

- piece counts and all ten raw PSQT rank/edge/center bases
- bishop pair
- doubled-extra, isolated, and squared passed-pawn advancement
- knight, bishop, rook, and queen mobility
- open and semi-open rook files
- king-shield pawns
- phase plus exact middlegame, endgame, White-relative, tempo, and side-to-move scores

The raw coefficients reconstruct every current constant and formula exactly. A deterministic 100,000-position oracle compares the reconstruction with production evaluation. The schema is accounting infrastructure, not permission to change weights or add terms.

NNUE remains deferred. A future evaluation interface may add a scalar incremental accumulator first, followed by exact scalar-versus-NEON tests. Training tooling must remain separate from the engine build.
