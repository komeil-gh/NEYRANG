# REKHNE H16: capture history

## Trigger evidence

A fresh H15-PGO depth-12 profile attributed 29.55% of sampled CPU time to the
staged MovePicker. NEYRANG already classifies tactical moves with exact SEE and
orders them with material, exchange, and the frozen SHEGERD policy, but it does
not remember which tactical moves caused cutoffs in the current search.

## Candidate

Add one searcher-local bounded capture-history table indexed by side, moving
piece, destination square, and captured-piece code. The no-capture code covers
quiet promotions that already live in the tactical stages.

The table may alter order only inside the existing good- or bad-tactical stage.
It must not override preferred moves, move a losing capture into the good stage,
or move any tactical move across killers or quiets. On a non-null beta cutoff,
reward a tactical cutoff move and penalize earlier searched tactical moves with
the same saturating depth bonus already used by quiet history. Qsearch does not
train the table.

## Gates

1. Focused tests must prove bucket separation, saturation, and unchanged stage
   boundaries. Formatting, Clippy, the full Rust/Python suites, Perft, tactical
   tests, and repository contracts must pass on SSH.
2. The native candidate must keep Perft exact and must not regress the
   interleaved depth-8 median by more than 5%. Search nodes and checksum may
   change because ordering is the feature under test.
3. A clean 1,000-game fixed-node match against the frozen H14 source baseline
   must have a non-negative point estimate. A pass opens a clean 1,000-game
   equal-time match under the same 500 reversed opening pairs; retention again
   requires a non-negative point estimate.
4. Only a retained candidate may receive a same-opening 1,000-game Blunder
   screen. Every match requires independent replay and zero candidate timing,
   legality, crash, or protocol anomaly.

## Outcome

Pending. No result or strength claim exists until every applicable gate above
has completed.
