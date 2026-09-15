# SHEGERD Game-Persistent Butterfly History

## Hypothesis

The UCI controller currently preserves the transposition table between moves
but constructs a fresh searcher, and therefore a zeroed butterfly history, for
every `go`. Keep the main worker's bounded history across moves in the same
game, clone it as the starting prior for Lazy-SMP helpers, and clear it on
`ucinewgame`. This lets SHEGERD reuse ordering evidence already paid for during
earlier searches without adding a new scoring model.

## Gate

H3r builds on retained H3o and changes no evaluation, pruning margin, or move
score formula. Require all tests and unchanged single-search depth-5/depth-8
trees. Then run 128 paired openings at 20,000 nodes against frozen H3o with the
same policy and `EvalMix=10`. Reject on a negative point estimate or any
candidate anomaly; only a pass permits a 256-game Blunder 7.6 screen.

## Outcome

The candidate preserved the 536,259-node depth-8 tree and checksum
`9d8d22b14e51010d`, and all 141 Rust tests passed. Its clean 128-game
fixed-node parent screen scored 31/37/60 (47.66%,
`-16.30 +/-45.96 Elo`). H3r failed the registered point-estimate floor, so no
Blunder games ran and the playing code was removed.
