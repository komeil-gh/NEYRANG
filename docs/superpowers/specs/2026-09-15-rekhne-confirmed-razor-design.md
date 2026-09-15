# REKHNE H4g: SANJ-confirmed shallow razor

## Hypothesis

The retained search still expands ordinary quiet moves at shallow non-PV nodes
whose static SANJ score is far below alpha even after quiescence resolves every
non-losing capture. Historical Blunder forensics place the first 100 cp error at
median depth 4, so removing only these already-failing shallow branches may buy
a completed iteration without weakening tactical defense.

## Candidate

At non-root, non-PV depths one through three, first require all existing safety
conditions: no check, no null subtree, no mate-score window, meaningful
non-pawn material, and no quiet TT move. When static SANJ is at least
`500 * depth` below alpha, run the unchanged qsearch at the same window. Return
its score only when qsearch also fails low; otherwise continue the complete main
search. This is stricter than the current Stockfish linear razor because SANJ
and tactical qsearch must agree before REKHNE prunes the quiet remainder.

No evaluation weight, ordering rule, artifact, time control, or UCI default
changes. The experiment is isolated from the rejected H4f delta-pruning code.

## Gates

1. Full Rust tests, the tactical suite, start-position perft 5, and Clippy must
   pass, including a direct fail-low/position-restoration check.
2. The five-position hybrid depth-8 tree must shrink by at least 5% without
   changing any final best move or score.
3. A passing engineering gate opens 128 fresh, disjoint color-reversed pairs at
   20,000 nodes against H4c. Reject on an anomaly or negative point estimate.
4. Only a passing parent screen opens a separate 256-game Blunder 7.6.0 screen;
   it must exceed the retained 26.56% point estimate to claim progress.

## Outcome

Rejected before the remote gate. The direct fail-low/restoration test passed and
H4g preserved all five hybrid depth-8 best moves and scores, but reduced the
tree only from 513,582 to 503,805 nodes (-1.90%). That missed the registered 5%
engineering floor, so no match was opened and the playing code was removed.
