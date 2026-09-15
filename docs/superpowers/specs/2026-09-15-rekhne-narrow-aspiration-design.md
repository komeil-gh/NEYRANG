# REKHNE H4x: narrow root aspiration

## Hypothesis

NEYRANG centers every depth-four-and-deeper root search on the completed prior
iteration, but opens a fixed 50-centipawn window. Current Stockfish also centers
on prior root evidence and starts much narrower before widening on failure:
<https://github.com/official-stockfish/Stockfish/blob/master/src/search.cpp>.
NEYRANG's simpler evaluator does not justify copying Stockfish's tuned values,
but halving its own initial window may reduce ordinary root work without
changing the full-window fallback.

## Candidate

Change only the initial aspiration delta from 50 to 25 centipawns at depth four
and above. Keep the previous completed score as the center, the existing
fail-soft comparisons, geometric widening, mate bounds, root preference,
search order, evaluator, pruning, history, artifacts, and UCI defaults.

## Gates

1. Add one direct boundary check for the registered initial delta. All Rust
   tests, the 11-position tactical suite, start-position perft 5, formatting,
   and Clippy must pass.
2. The five-position hybrid depth-8 tree must shrink by at least 1% with all
   best-move/score signatures unchanged. A tree increase or signature change
   rejects the candidate locally.
3. A passing local gate opens 128 fresh, disjoint color-reversed pairs at
   20,000 nodes against H4m. Reject on any anomaly or negative point estimate.
4. A separate fresh 128-pair `0.5+0.005` parent screen must also have a
   non-negative point estimate. Only both passing screens may open Blunder.

Failure removes the playing code while retaining the measured outcome.

## Result

The focused boundary regression passed, and the five-position hybrid depth-8
signature remained exact. The narrower window nevertheless increased the tree
from 513,582 to 579,345 nodes (+12.80%) and single-run elapsed time from 0.452
to 0.490 seconds. It therefore failed the registered local gate; no remote
build or match was opened, and the playing code plus temporary test were
removed.
