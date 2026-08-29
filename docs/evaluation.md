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

NNUE remains deferred. A future evaluation interface may add a scalar incremental accumulator first, followed by exact scalar-versus-NEON tests. Training tooling must remain separate from the engine build.
