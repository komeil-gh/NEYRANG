# SANJ Residual Ablation

## Hypothesis and gate

The selected 10% N7 residual beat its earlier policy parent, but its pure
network family remains weak and the current hybrid still scores poorly against
Blunder. Test classical SANJ (`EvalMix=0`) against the selected 10% hybrid with
the identical frozen H3o binary, network, stage-aligned policy, 20,000-node
budget, and 128 fresh paired openings.

Reject classical SANJ on a negative point estimate or any anomaly. Only a pass
opens a separate 256-game Blunder 7.6.0 screen on fresh openings. Promotion
requires that external point estimate to exceed the retained hybrid's 26.56%
screen; no intermediate mix or rescue weight may be selected from either set.

## Outcome

The clean 256-game ablation scored 66 wins, 100 losses, and 90 draws for
classical SANJ (43.36%, `-46.42 +/-32.70 Elo`). All 128 pairs completed with
empty stderr and zero exit status. The 10% residual remains selected; the
Blunder stage stayed closed.
