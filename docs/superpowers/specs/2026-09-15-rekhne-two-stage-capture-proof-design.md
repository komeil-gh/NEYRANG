# REKHNE H4p: two-stage capture proof

## Hypothesis

NEYRANG already spends exact SEE work to rank non-losing captures, but every
eligible main-search node still explores the full move set even when one of
those captures proves a value far above beta. A narrow capture-only proof can
turn that existing tactical information into a safe early cutoff without
changing SANJ, policy scoring, history, or ordinary move ordering.

Current Stockfish also verifies a tactical candidate with qsearch before a
reduced main search in its
[ProbCut step](https://github.com/official-stockfish/Stockfish/blob/master/src/search.cpp).
H4p adopts only that proof shape; its eligibility, fixed margin, returned
bound, and existing NEYRANG MovePicker are local to this engine.

## Candidate

At non-root, non-PV main-search nodes of depth at least five, outside check,
null subtrees, and mate-score windows, set the proof threshold to `beta + 200`.
Reuse the existing qsearch MovePicker to expose only promotions and exact
non-losing captures. For each candidate, require both a zero-window qsearch and
a zero-window `depth - 4` main search to reach that threshold. Only then return
the original beta lower bound. Restore position, repetition, and NNUE state
after every probe.

The 200-centipawn margin is twice NEYRANG's retained 100-centipawn shallow SEE
step; no Stockfish tuning constant is copied. There is no new TT format, move
class, history update rule, extension, or external artifact.

## Gates

1. A direct regression must prove a winning capture can complete both stages,
   produce a cutoff, and restore state. All Rust tests, 11 tactical cases,
   start-position perft 5, formatting, and Clippy must pass.
2. The five-position hybrid depth-8 best-move/score signature must stay exact
   and its combined tree must shrink by at least 3% versus H4m's 513,582 nodes.
3. A passing local gate opens 128 fresh, disjoint color-reversed pairs at
   20,000 nodes against H4m. Reject on any anomaly or negative point estimate.
4. Only a passing parent screen opens a separate 256-game Blunder 7.6.0 screen.
   External progress requires exceeding the retained 26.56% point estimate.

Failure removes the playing code while preserving the measured result.

## Outcome

The direct regression completed both proof stages, returned the original beta
bound, and restored position state. All 143 Rust tests, 11 tactical cases,
start-position perft 5 (`4,865,609`), formatting, and Clippy passed. The hybrid
depth-8 signature stayed exact across all five positions while the combined
tree fell from 513,582 to 376,308 nodes (`-26.73%`).

The clean 256-game parent screen scored 76 wins, 103 draws, and 77 losses
(49.80%, `-1.36 +/-33.65 Elo`). All 128 opening pairs and 25,912 plies replayed
successfully; all games terminated normally with no warning, timing, legality,
crash, or protocol anomaly. The negative point estimate fails the registered
parent gate, so no Blunder screen is opened. H4p is rejected and its playing
code is removed despite the large tree reduction.
