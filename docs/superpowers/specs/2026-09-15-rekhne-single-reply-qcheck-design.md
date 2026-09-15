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

## Outcome

Rejected. The direct quiet-mate/state-restoration regression passed, as did
all 143 Rust tests, the 11-position tactical suite, start-position perft 5
(4,865,609 nodes), formatting, and Clippy.

At hybrid depth 8, H4r searched 481,916 nodes versus H4m's 513,582
(-6.17%). Four of five move/score signatures were identical; the initial
position changed from `d2d4/23` to `b1c3/23`. Elapsed time nevertheless rose
from 0.482 s to 0.623 s (+29.3%) because every eligible qsearch node scanned
and made quiet moves to discover the rare forcing checks.

The clean 256-game fixed-node parent screen scored 92/94/70 and 54.30%
(`+29.93 +/-33.38 Elo`). Because the local timing regression could reverse
that result under real clocks, a separate fresh 128-pair equal-time screen at
`0.5+0.005` was run before the external gate. It was also clean but scored
56/85/115 and 38.48% (`-81.54 +/-34.51 Elo`). No Blunder screen was opened.

The playing code was removed. Any future qsearch-check experiment must obtain
checking candidates without a full legal quiet-move make/unmake scan.
