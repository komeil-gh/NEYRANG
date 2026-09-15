# REKHNE H4f: recapture-aware qsearch delta pruning

## Hypothesis

The retained H4c parent spends search budget resolving captures that cannot
raise alpha even after the captured material and a conservative positional
allowance are added to stand pat. Historical Blunder forensics locate the first
100 cp error at median ply 26 and median depth 4, so the goal is to buy a deeper
completed iteration rather than add another evaluation term.

## Candidate

At non-check quiescence nodes only, skip a non-promotion capture when all of the
following hold:

- it does not give check;
- it is not an immediate recapture on the previous move's destination;
- alpha is outside the mate band; and
- `stand_pat + captured_piece_value + 300 <= alpha`.

The existing staged picker remains authoritative, including its exact non-losing
SEE requirement. Check evasions, promotions, recaptures, legal terminal
detection, and every capture capable of reaching the conservative delta bound
remain unchanged.

## Gates

1. Full Rust tests, tactical regression tests, perft depth 5, and clippy must pass.
2. The five-position depth-8 tree must shrink without changing its legal output
   contract; record nodes, checksum, and elapsed time against frozen H4c.
3. If the engineering screen is useful, freeze one candidate and run 128 fresh,
   disjoint color-reversed opening pairs at 20,000 nodes against H4c. Reject on
   any anomaly or a negative point estimate.
4. Only a passing parent screen may open a separate 256-game Blunder screen.

The experiment is isolated. A failed gate removes the playing code and records
the rejection; it is never combined with another search change.
