# SHEGERD P3: Teacher-Residual Policy

## Hypothesis

Current H18 losses are dominated by quiet-to-quiet choices at depths five and
six. A bounded teacher residual can improve within-stage ordering without
discarding the general P2 policy that already survived game testing.

## Frozen change

Label current H18-versus-Blunder decisions with Stockfish 18 at 20,000 nodes,
fit the existing version-2 additive policy format, then blend exactly 25% of
the fitted integer tables with 75% of P2. No search rule, feature schema,
stage boundary, SANJ weight, or UCI behavior changes.

## Acceptance

1. The artifact must pass checksum, bounds, and held-out policy audits.
2. A fixed-node parent match on fresh paired openings must have a positive
   point estimate and no anomaly.
3. A larger fixed-node confirmation must use openings disjoint from training
   and the first screen, reach LOS above 95%, and pass independent PGN audit.
4. A same-opening Blunder comparison must improve over frozen H18-PGO.

## Result

The full teacher replacement failed at 46.00% over 512 parent games. The sole
25% residual candidate scored 52.73% in its 512-game parent screen and 51.86%
(`+12.92 +/-13.99 Elo`, LOS 96.51%) over 1,372 disjoint confirmation games.
Against Blunder on the same 256 paired openings, frozen H18-PGO scored 28.42%
and P3 scored 33.79%. All retained games completed without engine or protocol
failure, so P3 replaced the embedded P2 artifact.
