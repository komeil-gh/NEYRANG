# REKHNE H5m: bounded ProbCut

## Hypothesis

A small tactical pre-cut at deep non-PV nodes may avoid full searches when a
forcing capture already proves a score well above beta. The candidate must
improve play against the retained embedded-default parent; tree reduction alone
is not sufficient.

## Candidate

At non-root, non-PV nodes outside check and null subtrees from depth five, try
at most three non-losing tactical moves. A quiescence probe must first reach
`beta + 180`; only then may a depth-reduced null-window search confirm the
cutoff. Mate windows are excluded. Position, hash, and incremental evaluator
state must be restored after every probe.

No evaluator, move ordering, UCI default, artifact, or other pruning rule is
changed.

## Gates

1. Full Rust tests, tactical tests, formatting, Clippy, release build, and a
   focused cutoff/restoration regression must pass.
2. The candidate must produce a positive point estimate in 512 fresh paired
   games against the retained parent at 20,000 nodes.
3. Only a passing parent screen may open a separate Blunder 7.6.0 screen.

## Outcome

Rejected. All correctness and build gates passed on the SSH worker. The audited
parent screen completed 512 normal games with 154 wins, 200 draws, and 158
losses: 49.61%, `-2.71 +/-20.88 Elo`. It used 256 color-reversed opening pairs,
20,000 nodes, one default engine thread, 64 MiB hash, eight concurrent games,
and had no crash, warning, timeout, protocol, legality, or replay anomaly.

The negative point estimate failed the registered gate, so no Blunder screen
was opened and the playing code plus focused test were removed. The retained
engine remains byte-for-byte based on commit `a2ce55a` until another candidate
earns promotion through games.
