# REKHNE H4o: check-aware late-move pruning

## Hypothesis

H3f's registered contract preserves checking moves, but the retained search
currently drops the entire remaining quiet stage as soon as its shallow
late-move threshold is reached. A quiet check after that boundary is therefore
never inspected. Testing each late quiet only for immediate check restores the
registered tactical exception without admitting ordinary late quiets.

## Candidate

At the existing non-root, non-PV depth-one-through-three late-move boundary,
make each remaining quiet solely to determine the exact resulting check state.
Search it through the unchanged PVS path only when it checks; otherwise restore
the position immediately and continue. Preferred moves, killers, captures,
promotions, nodes in check, mate defense, thresholds, SANJ/NNUE, policy,
history, and every other pruning rule remain frozen.

## Gates

1. A direct regression must prove that a checking quiet beyond the late-move
   boundary is searched and that position state is restored. Full Rust tests,
   the tactical suite, start-position perft 5, formatting, and Clippy must pass.
2. The five-position hybrid depth-8 tree may grow by at most 15%. Record every
   changed best move or score; reject any tactical regression.
3. A passing local gate opens 128 fresh, disjoint color-reversed pairs at
   20,000 nodes against H4m. Reject on any anomaly or negative point estimate.
4. Only a passing parent screen opens a separate 256-game Blunder 7.6.0 screen.
   External progress requires exceeding the retained 26.56% point estimate.

The experiment is isolated from the rejected H4l qsearch checks and H4n losing
capture checks. Failure removes the playing code while preserving the result.

## Engineering result

The direct regression passed and observed four late quiet checks reaching the
normal search path. All 142 Rust tests, 11 tactical cases, start-position perft
5 (`4,865,609`), formatting, and Clippy passed. Against H4m at hybrid depth 8,
the candidate searched 523,847 nodes versus 513,582 (`+2.00%`), below the 15%
ceiling. Four of five best-move/score pairs were unchanged; the rook ending
changed from `b4f4 / 82` to `b4c4 / 72`. The candidate advances to the parent
screen; no strength claim is made from these local measurements.
