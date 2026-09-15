# REKHNE H5i: guarded PV late-move reduction

## Hypothesis

NEYRANG searches every late move at full depth inside PV nodes, even though the
same move first receives a null-window probe and is re-searched whenever it
raises alpha. A guarded one-ply reduction for demonstrably late, quiet PV moves
should recover depth without trusting the reduced result as a principal line.

## Candidate

Keep root moves, the first six searched moves, captures, promotions, checks,
TT moves, killers, and positive-history quiets at full depth. At a non-root PV
node of depth five or greater, reduce later quiets by one ply. Any reduced
alpha raise keeps the existing full-depth null-window re-search and ordinary
PV re-search. Non-PV LMR, SANJ, SHEGERD ordering, pruning margins, artifacts,
and UCI defaults remain unchanged.

This is a deliberately narrow adaptation of the reduced-first, full-depth-on-
raise pattern used by current alpha-beta engines. It does not import another
engine's reduction table or tuned constants.

## Gates

1. A focused regression must prove that the PV-only path activates and restores
   the position. All Rust tests, the tactical suite, perft, formatting, and
   Clippy must pass.
2. The frozen five-position hybrid depth-8 tree must shrink by at least 2%
   without a tactical regression.
3. A passing tree opens a fresh 128-pair, 20,000-node match against frozen H4m.
   Reject on any anomaly or negative point estimate.
4. Only a passing parent screen opens a separate Blunder 7.6.0 screen. External
   progress requires exceeding the retained 26.56% point estimate.

## Outcome

The focused regression, all 11 tactical tests, and the deterministic gate
passed. H5i reduced the frozen hybrid depth-8 tree from 513,582 to 440,417
nodes (-14.25%) with all five best-move/score signatures unchanged. Its fresh
256-game fixed-node parent screen was exactly even at 76/104/76 (50.00%), and
a separate 256-game `0.1+0.001` screen scored 83/96/77 (51.17%,
`+8.14 +/-31.09 Elo`).

The independent Blunder screen scored 28/63/165 (23.24%,
`-207.54 +/-36.06 Elo`), below the retained 26.56% external point estimate.
One Blunder-only threefold-PV warning occurred; no NEYRANG protocol failure,
crash, timeout, or illegal move was reported. Per the registered external gate,
H5i is rejected and its playing code is removed.
