# REKHNE H4n: losing-check capture bridge

## Hypothesis

NEYRANG's qsearch discards every negative-SEE capture before the search can
observe whether it forces a reply. The historical Blunder diagnostics contain
catastrophic shallow mate misses, while H4l proved that admitting quiet checks
is far too broad. A negative-SEE capture that immediately checks the opponent
is the narrow forcing class between those two designs.

## Candidate

After the normal good-tactical qsearch stage is exhausted outside check, expose
the already-classified bad captures one at a time. Make each move only to test
the exact resulting check state. Search it through the unchanged qsearch only
when it checks; otherwise restore the position immediately. Positions already
in check retain their complete legal-evasion search.

Exact SEE values and ordering are reused; quiet checks, non-checking bad
captures, main search, SANJ/NNUE, policy, pruning margins, artifacts, and UCI
defaults remain frozen. There is no external threshold or copied tuning value.

## Gates

1. A direct regression must prove one negative-SEE checking capture crosses the
   bridge while an otherwise comparable negative-SEE non-checking capture does
   not. Full Rust tests, the 11-position tactical suite, start-position perft 5,
   formatting, and Clippy must pass.
2. The five-position hybrid depth-8 tree may grow by at most 15%. Every changed
   best move or score must be recorded, and no tactical fixture may regress.
3. A passing local gate opens 128 fresh, disjoint color-reversed pairs at
   20,000 nodes against H4m. Reject on any anomaly or negative point estimate.
4. Only a passing parent screen opens a separate 256-game Blunder 7.6.0 screen;
   it must exceed the retained 26.56% point estimate to claim external progress.

