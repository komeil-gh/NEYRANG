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

## Engineering result

All 142 Rust tests, the 11-test tactical suite, start-position perft 5, and
Clippy passed. H4f preserved all five hybrid depth-8 best moves and scores while
reducing their combined tree from 513,582 to 474,385 nodes (-7.63%). Across 21
interleaved runs per binary, median time improved from 0.445794 to 0.429091
seconds (+3.89% throughput). A 20-position replay of the historical
Blunder-error set produced the same move and completed-depth sequence as H4c.

## Match outcome

The fresh 256-game parent screen completed with zero exit status, empty stderr,
128 color-reversed pairs, and all 256 terminations marked normal. H4f scored 68
wins, 107 draws, and 81 losses: 47.46% (`-17.66 +/-27.47 Elo`). Its negative
point estimate failed the registered floor, so the separate Blunder screen was
not opened and the playing code was removed.
