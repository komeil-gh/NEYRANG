# REKHNE H4s: TT-guided PV qsearch bridge

## Hypothesis

NEYRANG's Blunder forensics place most catastrophic decisions at completed
depths three through five. H4r proved that resolving one more quiet forcing
ply can improve fixed-node play, but discovering checks by scanning every
quiet move was much too expensive under equal time.

H4s instead spends the extra ply only where iterative deepening has already
left a deeper principal move in the transposition table. Current Stockfish
also preserves at least one full-search ply for a sufficiently deep TT move
when a PV re-search is about to enter qsearch; NEYRANG adopts only that narrow
structural condition, without its tuned search ecosystem.

## Candidate

At a PV node with one nominal ply remaining, search the TT move's child at
depth one instead of qsearch only when the existing TT entry is deeper than
one ply. All other moves and every zero-window search keep the original depth.
The branch performs no new move generation, evaluation, SEE, or table probe.

Mate handling, qsearch, SANJ/NNUE, SHEGERD ordering, history, reductions,
pruning, artifacts, and UCI defaults remain unchanged. Synthetic null
subtrees remain excluded because they do not probe the TT.

## Gates

1. A direct regression must prove that only the eligible TT-guided PV move
   crosses the depth-zero boundary. All Rust tests, the 11-position tactical
   suite, start-position perft 5, formatting, and Clippy must pass.
2. The five-position hybrid depth-8 tree may grow by at most 20%; no tactical
   fixture may regress and every changed best move or score is recorded.
3. A passing local gate opens 128 fresh, disjoint color-reversed pairs at
   20,000 nodes against H4m. Reject on any anomaly or negative point estimate.
4. Because the feature adds search, a separate fresh 128-pair equal-time
   `0.5+0.005` parent screen must also have a non-negative point estimate.
5. Only both passing parent screens open a separate 256-game Blunder 7.6.0
   screen; external progress requires exceeding the retained 26.56% score.

Failure removes the playing code while preserving the measured result.
