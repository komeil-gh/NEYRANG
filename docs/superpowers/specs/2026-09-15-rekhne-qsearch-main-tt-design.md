# REKHNE H4j: main-bound reuse in qsearch

## Hypothesis

REKHNE already pays for a verified transposition-table result at a full-search
node, but qsearch ignores that result when the same position is reached through
a capture transposition. Reusing a sufficiently deep main-search bound can
avoid repeating the tactical subtree without adding another pruning heuristic.

## Candidate

After stop and draw adjudication, qsearch may probe the existing TT when outside
a synthetic null subtree. It may return only an exact score or a bound that
already closes the current window, with the same mate-score normalization used
by main search. The TT move may lead the existing staged picker only when it is
a legal qsearch candidate: any legal evasion while in check, otherwise a capture
or promotion.

H4j never writes a qsearch entry, so a shallow tactical result cannot replace a
deeper main-search entry. Move generation, exact SEE, SANJ/NNUE evaluation,
policy, pruning margins, artifacts, and UCI defaults remain unchanged.

## Gates

1. Full Rust tests, the tactical suite, start-position perft 5, and Clippy must
   pass, including exact/lower/upper TT cutoff and quiet-preferred exclusion.
2. The five-position hybrid depth-8 tree must shrink by at least 1% without
   changing any final best move or score; measure 21 interleaved runs.
3. A passing engineering gate opens 128 fresh, disjoint color-reversed pairs at
   20,000 nodes against H4c. Reject on an anomaly or negative point estimate.
4. Only a passing parent screen opens a separate 256-game Blunder 7.6.0 screen;
   it must exceed the retained 26.56% point estimate to claim progress.
