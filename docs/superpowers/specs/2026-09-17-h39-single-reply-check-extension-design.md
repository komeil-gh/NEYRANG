# H39: single-reply check extension

## Trigger evidence

NEYRANG's earlier extension of every in-check node grew the registered tree by
17.67% and reduced its external Blunder point estimate, so it was removed.
REKHNE still spends one nominal ply on a forced evasion even when the checked
side has exactly one legal move. That narrow case carries no move-choice
branching and can push the first real choice beyond the horizon.

## Candidate

At a main-search node that is in check and has exactly one legal move, search
that sole child without consuming the current nominal ply. Do not extend
multi-reply checks or quiescence, and do not change SANJ, ordering, pruning,
reductions, artifacts, or UCI defaults. Draw, stop, mate-distance, and maximum
ply guards remain authoritative.

## Gates

1. A focused regression must distinguish the exact one-reply checked case from
   multi-reply, non-check, and terminal nodes. The complete quality suite,
   Perft, tactical suite, UCI smoke, and deterministic benchmark must pass.
2. The depth-8 benchmark tree may grow by at most 5% relative to H35.
3. A clean 512-game paired 50,000-node parent screen against frozen H35 must
   score at least 50%, followed by a clean 512-game paired `0.5+0.005` screen
   whose point estimate is non-negative.
4. Only both parent passes open the comparable 512-game Blunder 7.6.0 screen.
   Retention requires exceeding H35's 33.30% point estimate without a candidate
   warning, crash, timeout, protocol, legality, or opening-pair anomaly.

## Outcome

Rejected and reverted.

The complete quality suite, focused forced-evasion regression, Perft 5, and
tactical tests passed. The depth-8 tree grew from 536,259 to 543,405 nodes
(+1.33%), within the 5% engineering ceiling. The strict 512-game fixed-node
parent screen then scored 161 wins, 206 draws, and 145 losses: 51.56%,
`+10.86 +/- 19.72 Elo`.

The decisive equal-time parent screen reversed that result. H39 scored 130
wins, 217 draws, and 165 losses: 46.58%, `-23.79 +/- 20.74 Elo`, with both
engines at one thread, 64 MiB hash, `0.5+0.005`, and identical colour-reversed
opening pairs. The strict audit passed. The Blunder gate was not opened. This
candidate is further evidence that node-limited gains cannot justify a search
extension whose wall-time cost reduces practical depth.
