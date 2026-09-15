# REKHNE H5a: margin-conditioned deep null move

## Hypothesis

NEYRANG's retained NMP uses R3 at every eligible depth-six-and-deeper node,
regardless of how far SANJ already stands above beta. Current Stockfish makes
null reduction depend on both depth and the evaluation margin over beta:
<https://github.com/official-stockfish/Stockfish/blob/master/src/search.cpp>.
A deliberately coarse NEYRANG rule can spend one less ply only where its own
existing gate already has a large fail-high margin.

## Candidate

Keep every retained NMP eligibility guard. Use R4 only at depth seven or deeper
when the already-computed static evaluation is at least 300 centipawns above
beta; keep R3 at other depth-six-plus probes and R2 below depth six. The null
window, mate bounds, zugzwang material guard, one-null-per-line rule, evaluator,
ordering, and all other pruning remain unchanged.

## Gates

1. A direct reduction-boundary regression plus all Rust tests, tactical tests,
   perft 5, formatting, and Clippy must pass.
2. The five-position hybrid depth-8 tree must shrink by at least 2% without any
   best-move/score signature change.
3. A passing local gate opens fresh independent 256-game fixed-node and
   `0.5+0.005` parent screens against H4m. Both require non-negative point
   estimates and zero anomalies before any Blunder screen.

Failure removes the playing code while retaining the measured outcome.
