# REKHNE Tactical Confirmation

## Hypothesis

At a deep non-PV node already close to beta, a small number of exact-SEE-safe
tactical moves can cheaply prove that the node fails high. Requiring both the
existing quiescence search and a reduced main search avoids trusting a capture
score or static SANJ alone.

## Boundary and gate

H3u builds on retained H3o. It tries at most three good tactical moves at
depth five or deeper, outside check, root, PV, mate windows, and synthetic null
subtrees. Static SANJ must be within 160 centipawns of beta. A candidate first
passes the raised `beta + 160` threshold in qsearch, then confirms it at
`depth - 4`; only the confirmed result cuts off. Existing MovePicker stages,
exact SEE, evaluator, history, LMR, NMP, and other pruning margins are unchanged.

Require all Rust tests and Clippy, then at least a 2% reduction in the frozen
depth-8 hybrid tree without a tactical-suite regression. A passing tree gate
opens a fresh 128-pair, 20,000-node screen against frozen H3o using eight match
workers. Reject on a negative candidate point estimate or any candidate
anomaly; only a pass permits a separate Blunder 7.6 screen.

## Tree gate

All 142 Rust tests, the 11-test tactical suite, and Clippy passed. With the
frozen N7 network, stage-aligned policy, and `EvalMix=10`, H3u reduced the
five-position depth-8 tree from 513,582 to 354,534 nodes (-30.97%) while
preserving all five best moves and reported scores. The registered parent
screen is therefore open.
