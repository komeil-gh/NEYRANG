# REKHNE H4l: safe qsearch forcing bridge

## Hypothesis

NEYRANG's capture-only qsearch can stop immediately before a sound quiet check,
leaving short searches blind to a forced reply. H3m showed that extending every
checked main-search node is too broad. H4l instead permits one non-capturing,
non-promotion check per qsearch line, and only when exact threshold SEE proves
the moved piece is not lost.

## Candidate

After the established capture/promotion stage is exhausted at a non-check
qsearch node, inspect remaining legal quiet moves in generator order. Search a
move only if `see_ge(move, 0)` passes and the resulting position checks the
opponent. The child consumes the line's single quiet-check budget; its forced
evasions remain covered by the existing in-check qsearch path. Synthetic null
subtrees cannot use the bridge.

Stand pat, exact tactical ordering, losing-capture pruning, SANJ/NNUE, policy,
main search, artifacts, and UCI defaults remain unchanged. There is no quiet
check scoring table, repeated checking extension, or copied engine margin.

## Gates

1. Full Rust tests, the 11-position tactical suite, start-position perft 5,
   Clippy, and a direct one-budget/restoration regression must pass.
2. The five-position hybrid depth-8 tree may grow by at most 30%; no solved
   tactical position may regress and every changed move/score is recorded.
3. A passing local gate opens 128 fresh, disjoint color-reversed pairs at
   20,000 nodes against H4c. Reject on an anomaly or negative point estimate.
4. Only a passing parent screen opens a separate 256-game Blunder 7.6.0 screen;
   it must exceed the retained 26.56% point estimate to claim progress.
