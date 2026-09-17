# H33 history-protected late-move pruning

## Hypothesis

Retained H3f stops the entire quiet stage after `3 + depth^2` searched moves,
even when the search-local butterfly history has positive evidence for a later
quiet. The existing LMR gate already trusts strong history, but the earlier LMP
gate can prevent that move from reaching LMR. Protecting only positive-history
quiets should repair this mismatch without adding a table or changing ordering.

## Frozen candidate

Keep every retained H30 rule and margin. At an otherwise eligible H3f late-move
cutoff, stop the quiet stage only when the current move's butterfly-history score
is non-positive. Preferred moves, killers, captures, promotions, checks, root,
PV, mate and in-check guards remain unchanged.

## Gates

1. Add one boundary test proving positive history survives while zero and
   negative history remain prunable. Run the complete correctness suite.
2. Run 512 color-reversed games at 50,000 nodes per engine against frozen H30.
   Reject on an anomaly or a score below 50%.
3. A passing candidate must score at least 50% in 512 equal-time parent games.
4. Only then run the same-opening equal-resource Blunder screen. It must exceed
   H30's registered 33.79% point estimate without an anomaly.

## Outcome

Rejected and reverted. The 512-game fixed-node parent gate scored 146 wins,
226 draws and 140 losses: 50.59%, +4.07 +/-20.28 Elo, with pentanomial
`[15, 55, 108, 65, 13]`. The required equal-time confirmation then scored
158 wins, 194 draws and 160 losses: 49.80%, -1.36 +/-23.22 Elo, with
pentanomial `[21, 67, 84, 61, 23]`. Both runs completed all 512 games without
warnings, crashes or timeouts. Because equal-time score was below the frozen
50% gate, no Blunder screen was run and the candidate code was removed.
