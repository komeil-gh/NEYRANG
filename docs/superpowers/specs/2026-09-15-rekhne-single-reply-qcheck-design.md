# REKHNE H4r: single-reply qsearch checks

## Hypothesis

The 1,000-game Blunder forensics place catastrophic errors mainly at completed
depths three through five. H4l tried to bridge capture-only qsearch with quiet
checks, but its `see_ge(move, 0)` guard could not distinguish them because
NEYRANG SEE intentionally returns zero for ordinary quiet moves. It therefore
admitted a broad checking tree and failed even the local time ceiling.

H4r replaces that ineffective guard with an exact forcing property: search a
quiet check only when the checked side has zero or one legal reply. This is a
NEYRANG-specific tactical bridge based on legal geometry, not a style bonus or
an imported tuning table.

## Candidate

After ordinary capture/promotion qsearch is exhausted outside check, inspect
legal non-capturing, non-promotion moves. Make each move to determine exact
check state. Search it only if it checks and the resulting position has at most
one legal response. A qsearch line may cross this bridge once; captures before
the bridge retain the budget, while the admitted quiet check consumes it.
Null subtrees cannot use the bridge.

Stand pat, every existing evasion, exact tactical ordering, SEE capture
pruning, SANJ/NNUE, policy, main search, artifacts, and UCI defaults remain
unchanged. Position, repetition, and accumulator state must be restored after
every inspected move.

## Gates

1. A direct regression must prove a quiet mating check crosses the bridge once
   and restores state. All Rust tests, 11 tactical cases, start-position perft
   5, formatting, and Clippy must pass.
2. The five-position hybrid depth-8 tree may grow by at most 30%; no tactical
   fixture may regress and every changed best move or score is recorded.
3. A passing local gate opens 128 fresh, disjoint color-reversed pairs at
   20,000 nodes against H4m. Reject on any anomaly or negative point estimate.
4. Only a passing parent screen opens a separate 256-game Blunder 7.6.0 screen.
   External progress requires exceeding the retained 26.56% point estimate.

Failure removes the playing code while preserving the measured result.
