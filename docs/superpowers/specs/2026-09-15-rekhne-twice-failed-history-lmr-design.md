# REKHNE H4v: twice-failed-history deep LMR

## Hypothesis

H4u's `-MAX/8` threshold never activated because NEYRANG's bounded history
updates are much smaller: one depth-6, depth-7, or depth-8 penalty is 576, 784,
or 1,024 before gravity. A threshold at `-MAX/16` therefore represents roughly
two deep failures of the same side/from/to move instead of an unreachable
absolute score.

## Candidate

Keep retained LMR eligibility unchanged. At depth six or greater, reduce a
late quiet by one additional ply only when its existing butterfly history is
strictly below `-HistoryTable::MAX_SCORE / 16`. A raised reduced result keeps
the unchanged normal-depth zero-window re-search and ordinary PVS re-search.

No history update, table, move score, stage boundary, evaluator, qsearch rule,
pruning margin, artifact, or UCI default changes. The threshold is derived from
NEYRANG's existing update scale; it is not copied from another engine.

## Gates

1. A focused regression covers depth and score boundaries. All Rust tests,
   the 11-position tactical suite, start-position perft 5, formatting, and
   Clippy must pass.
2. The five-position hybrid depth-8 tree must shrink by at least 2%, and no
   tactical fixture may regress. Every changed move or score is recorded.
3. A passing local gate opens 128 fresh, disjoint color-reversed pairs at
   20,000 nodes against H4m. Reject on any anomaly or negative point estimate.
4. A separate fresh 128-pair equal-time `0.5+0.005` parent screen must also
   have a non-negative point estimate.
5. Only both passing parent screens open a separate 256-game Blunder 7.6.0
   screen; external progress requires exceeding the retained 26.56% score.

Failure removes the playing code while preserving the measured result.
