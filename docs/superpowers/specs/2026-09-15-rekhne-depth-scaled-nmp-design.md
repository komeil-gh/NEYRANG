# REKHNE H5j: depth-scaled null-move reduction

## Hypothesis

NEYRANG uses the same three-ply null reduction at every depth from six upward.
That spends increasing work proving the same fail-high as nominal depth grows.
A coarse depth-scaled reduction can recover search depth while every existing
material, check, PV, mate-window, and one-null-per-line guard remains intact.

## Candidate

Keep R2 below depth six. Use `R = 3 + depth / 6` from depth six upward: R4 at
depth 6-11, R5 at 12-17. Do not add an evaluation-margin term, verification
exception, or copied tuned constant. The null window, terminal checks, SANJ,
SHEGERD, ordinary move search, and all other pruning remain unchanged.

## Gates

1. A direct boundary regression, all Rust tests, the tactical suite, perft,
   formatting, and Clippy must pass.
2. The frozen hybrid depth-8 tree must shrink by at least 2% with no tactical
   regression.
3. A passing tree opens fresh 256-game fixed-node and equal-time parent screens;
   both require non-negative point estimates and no NEYRANG anomaly.
4. Only both parent passes open an independent Blunder 7.6.0 screen, which must
   exceed the retained 26.56% point estimate.

## Outcome

The direct boundary regression and all 11 tactical tests passed. H5j reduced
the frozen hybrid depth-8 tree from 513,582 to 463,510 nodes (-9.75%) without
changing the five best-move/score signatures. Its fresh 256-game fixed-node
parent screen then scored 81/85/90 (48.24%, `-12.22 +/-25.92 Elo`). The
registered non-negative parent floor failed, so no equal-time or Blunder screen
was opened. H5j is rejected and its playing code is removed.
