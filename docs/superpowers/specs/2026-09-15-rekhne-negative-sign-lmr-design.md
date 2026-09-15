# REKHNE H4w: negative-sign deep LMR

## Hypothesis

H4u and H4v proved that magnitude thresholds do not activate in NEYRANG's
depth-8 workload. Butterfly history is search-local and sparse, so its useful
signal here may be categorical: negative means this quiet has already failed;
zero means it has not accumulated evidence.

## Candidate

Keep retained LMR eligibility unchanged. At depth six or greater, reduce a
late quiet by one additional ply only when its existing butterfly-history
score is negative. A raised reduced result keeps the unchanged normal-depth
zero-window re-search and ordinary PVS re-search.

No history update, table, move score, stage boundary, evaluator, qsearch rule,
pruning margin, artifact, or UCI default changes. This is the final registered
history-conditioned reduction variant; no further threshold search follows a
failed local gate.

## Gates

1. A focused regression covers depth and sign boundaries. All Rust tests, the
   11-position tactical suite, start-position perft 5, formatting, and Clippy
   must pass.
2. The five-position hybrid depth-8 tree must shrink by at least 2%, and no
   tactical fixture may regress. Every changed move or score is recorded.
3. A passing local gate opens 128 fresh, disjoint color-reversed pairs at
   20,000 nodes against H4m. Reject on any anomaly or negative point estimate.
4. A separate fresh 128-pair equal-time `0.5+0.005` parent screen must also
   have a non-negative point estimate.
5. Only both passing parent screens open a separate 256-game Blunder 7.6.0
   screen; external progress requires exceeding the retained 26.56% score.

Failure removes the playing code while preserving the measured result.
