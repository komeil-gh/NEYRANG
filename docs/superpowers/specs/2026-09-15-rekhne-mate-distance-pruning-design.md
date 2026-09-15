# REKHNE H4m: exact mate-distance pruning

## Hypothesis

Once an ancestor has proved a shorter mate, a descendant cannot improve that
mate score: the best possible win is mate on its next move and the worst loss
is mate at the current ply. Clamping a non-root main-search window to those
exact bounds can avoid searches whose result cannot affect the root decision.

## Candidate

After stop/draw handling and the depth-zero handoff, clamp non-root main-search
`alpha` to `-VALUE_MATE + ply` and `beta` to `VALUE_MATE - ply - 1`. Return the
clamped alpha when the window closes. Establish `original_alpha` only after the
clamp so transposition-table bounds describe the window actually searched.

Qsearch, root windows, terminal mate scores, TT mate normalization, SANJ/NNUE,
ordering, policy, pruning margins, artifacts, and UCI defaults remain frozen.
The bounds are derived from NEYRANG's existing mate-score encoding; no external
engine constant or tuned margin is copied.

## Gates

1. A direct regression must prove both winning- and losing-mate window cutoffs
   occur before move generation and leave the position unchanged. Full Rust
   tests, the 11-position tactical suite, start-position perft 5, and Clippy
   must pass.
2. The five-position hybrid depth-8 best moves and scores must be identical to
   H4c, and the combined tree may not grow.
3. Retain only if a forced-mate local workload demonstrates an actual node
   reduction. Otherwise remove the source change without spending remote games.
4. A retained local optimization may open a fresh paired parent screen; only a
   non-negative point estimate can open a separate Blunder 7.6.0 screen.

## Outcome

Retained as an exact local optimization. All 142 Rust tests, including the
direct pre-move-generation cutoff regression, passed; the 11-test tactical
suite, start-position perft 5, formatting, and Clippy also passed. The
five-position hybrid depth-8 workload preserved every best move and score and
remained exactly 513,582 nodes.

Across three forced-mate positions, the candidate preserved mate distances and
best moves while reducing the combined tree from 9,561 to 7,100 nodes
(-25.74%). No remote games were opened because the general hybrid tree was
unchanged; this result is exact pruning/throughput evidence, not an Elo claim.
