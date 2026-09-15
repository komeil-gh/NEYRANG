# REKHNE H4t: TT-guided pruning evaluation

## Hypothesis

Reverse futility, null move, and forward futility currently decide from raw
SANJ even when the transposition table already holds a compatible search bound
for the same position. Reusing that paid evidence can make shallow pruning less
dependent on a single static estimate without another evaluator call or table
probe.

Current Stockfish likewise uses a non-decisive TT value as a better static
estimate only when the stored bound supports the direction of replacement.
H4t adopts that invariant, not its tuned margins or correction-history system.

## Candidate

When REKHNE first evaluates a main-search node for pruning, replace raw SANJ
with the TT score only in one of three cases: an exact non-mate score, a lower
bound above SANJ, or an upper bound below SANJ. Ignore mate-band values and
bounds that would move the estimate in the unsupported direction.

The adjusted value is local to reverse futility, null-move eligibility, and
forward futility. Leaf evaluation, returned static scores, qsearch, TT cutoff
rules, TT storage, SANJ/NNUE, SHEGERD ordering, history, reductions, artifacts,
and UCI defaults remain unchanged. Synthetic null subtrees remain excluded
because they do not probe the TT.

## Gates

1. A pure regression must cover exact, lower, upper, wrong-direction, and
   mate-band cases. All Rust tests, the 11-position tactical suite,
   start-position perft 5, formatting, and Clippy must pass.
2. The five-position hybrid depth-8 tree must shrink by at least 2%, may not
   grow, and no tactical fixture may regress. Every changed move or score is
   recorded.
3. A passing local gate opens 128 fresh, disjoint color-reversed pairs at
   20,000 nodes against H4m. Reject on any anomaly or negative point estimate.
4. A separate fresh 128-pair equal-time `0.5+0.005` parent screen must also
   have a non-negative point estimate.
5. Only both passing parent screens open a separate 256-game Blunder 7.6.0
   screen; external progress requires exceeding the retained 26.56% score.

Failure removes the playing code while preserving the measured result.

## Outcome

Rejected before remote testing. The bound-direction and mate-band regression,
all 143 Rust tests, the 11-position tactical suite, start-position perft 5,
formatting, and Clippy passed. All five hybrid depth-8 move/score signatures
also remained unchanged.

The candidate nevertheless grew the hybrid tree from 513,582 to 522,621 nodes
(+1.76%) and increased the single-run elapsed time from the 0.482 s H4m
reference to 0.700 s. It failed the predeclared requirement to shrink the tree
by at least 2% and not grow it. The playing code and temporary regression were
removed; no remote games ran.
