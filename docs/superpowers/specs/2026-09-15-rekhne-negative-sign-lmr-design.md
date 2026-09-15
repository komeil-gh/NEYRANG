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

## Result

The sign/depth regression, all 143 Rust tests, the 11-position tactical suite,
start-position perft 5 (4,865,609 nodes), formatting, and Clippy passed. At
hybrid depth 8, H4w searched 481,103 nodes versus H4m's 513,582 (-6.32%) with
all five move/score signatures unchanged; the single-run elapsed time was
0.430 s versus the 0.482 s H4m reference.

The clean, independently audited 20,000-node parent screen then scored
77/94/85 over 256 games: 124/256 (48.44%), -10.86 +/-20.56 Elo, with no
crash, timeout, protocol, forfeit, or warning anomaly. The fixed-node point
estimate is negative, so gate 3 fails. Per the registered ladder, the
equal-time and Blunder screens were not opened and the playing change was
removed.

Private evidence is retained under
`testing/private/h4w-negative-sign-lmr-20260915/`. The candidate executable
SHA-256 is
`c76066bf007ef77d5179d0e420fa9411baaceb3319d1759a9196f96148210bc9` and
the parent-opening SHA-256 is
`47401115a796d45f2f5cc59cd1013cee46684020561867e97a392447e017d669`.
