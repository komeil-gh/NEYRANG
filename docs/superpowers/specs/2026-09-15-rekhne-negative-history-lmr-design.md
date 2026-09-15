# REKHNE H4u: negative-history deep LMR

## Hypothesis

H4e showed that a second reduction ply saves substantial work but applying it
to every late deep quiet loses strength. REKHNE already owns stronger local
evidence: butterfly history becomes negative only after the same side/from/to
move repeatedly fails before another quiet causes a cutoff.

Current Stockfish also adjusts LMR depth from history evidence. H4u does not
copy its tuned formula; it uses NEYRANG's existing bounded table and one
derived threshold to isolate moves with demonstrated local failure.

## Candidate

Keep the retained one-ply LMR eligibility unchanged. At depth six or greater,
reduce by one additional ply only when the late quiet's existing butterfly
history is below `-HistoryTable::MAX_SCORE / 8`. A raised reduced result still
receives the unchanged normal-depth zero-window re-search, followed by the
ordinary PVS full-window re-search when required.

No table, update rule, move score, stage boundary, evaluator, qsearch rule,
pruning margin, artifact, or UCI default changes. TT moves, killers, early
moves, captures, promotions, checks, PV nodes, and strong-history quiets retain
their existing protection.

## Gates

1. A focused regression must cover both reduction depths at the exact history
   boundary. All Rust tests, the 11-position tactical suite, start-position
   perft 5, formatting, and Clippy must pass.
2. The five-position hybrid depth-8 tree must shrink by at least 5%, and no
   tactical fixture may regress. Every changed move or score is recorded.
3. A passing local gate opens 128 fresh, disjoint color-reversed pairs at
   20,000 nodes against H4m. Reject on any anomaly or negative point estimate.
4. A separate fresh 128-pair equal-time `0.5+0.005` parent screen must also
   have a non-negative point estimate.
5. Only both passing parent screens open a separate 256-game Blunder 7.6.0
   screen; external progress requires exceeding the retained 26.56% score.

Failure removes the playing code while preserving the measured result.

## Outcome

Rejected before remote testing. The reduction-boundary regression, all 143
Rust tests, the 11-position tactical suite, start-position perft 5, formatting,
and Clippy passed. The five-position hybrid depth-8 workload was nevertheless
inert: H4u and H4m both searched 513,582 nodes with identical move/score
signatures.

At `-HistoryTable::MAX_SCORE / 8`, the extra reduction did not activate in the
representative search. The playing code and temporary regression were removed;
no remote games ran.
