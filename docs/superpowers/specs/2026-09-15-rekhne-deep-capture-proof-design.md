# REKHNE H4q: deep two-stage capture proof

## Hypothesis

H4p preserved every local best move and removed 26.73% of the hybrid tree, but
lost its clean parent screen by one game. Its fixed `depth - 4` confirmation is
the only intentionally shallow part of the proof. Searching that second stage
one ply deeper should remove unsafe tactical cutoffs while retaining useful
tree savings.

The experiment keeps the capture-only qsearch confirmation shape described in
current [Stockfish search](https://github.com/official-stockfish/Stockfish/blob/master/src/search.cpp),
but changes exactly one H4p variable: reduced main-search depth becomes
`depth - 3`. NEYRANG's fixed 200-centipawn threshold, guards, MovePicker,
returned beta bound, and all unrelated search behavior stay frozen.

## Gates

1. The existing direct proof regression must pass with state restoration, as
   must all Rust tests, 11 tactical cases, start-position perft 5, formatting,
   and Clippy.
2. The five-position hybrid depth-8 best-move/score signature must stay exact
   and its combined tree must shrink by at least 10% versus H4m's 513,582 nodes.
3. A passing local gate opens 128 fresh, disjoint color-reversed pairs at
   20,000 nodes against H4m. Reject on any anomaly or negative point estimate.
4. Only a passing parent screen opens a separate 256-game Blunder 7.6.0 screen.
   External progress requires exceeding the retained 26.56% point estimate.

Failure removes the playing code while preserving the measured result.
