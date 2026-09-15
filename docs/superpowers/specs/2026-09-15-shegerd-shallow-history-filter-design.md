# SHEGERD H5e: shallow first-cut history filter

## Hypothesis

NEYRANG currently rewards a quiet move in butterfly history even when it is the
first move searched at depth one through three. That cutoff mostly confirms the
existing ordering and supplies little comparative evidence, while its reward
can distort later ordering. Ethereal historically retained the same narrow
filter at each of those three shallow depths in its official release history:
<https://github.com/AndyGrant/Ethereal/releases>.

## Candidate

Keep killer recording unchanged, but skip butterfly-history reward and malus
when the first searched quiet causes a beta cutoff at depth one through three.
All later quiet cutoffs and every cutoff from depth four onward train exactly as
before. Table size, gravity, bonuses, MovePicker stages, SANJ, pruning, artifacts,
time controls, and UCI defaults remain unchanged.

## Gates

1. A focused boundary test, all Rust tests, the tactical suite, start-position
   perft 5, formatting, and Clippy must pass.
2. The five-position hybrid depth-8 tree may grow by at most 5% with no solved
   tactical regression; every changed move or score is recorded.
3. A passing local gate opens fresh independent 256-game fixed-node and
   `0.5+0.005` parent screens against H4m. Both require non-negative point
   estimates and zero anomalies.
4. Only both passing parent screens open a fresh Blunder 7.6.0 screen, which
   must exceed the retained 26.56% point estimate.

Failure removes the playing code while retaining the measured outcome.

## Result

The focused boundary regression, all 143 Rust tests, the 11-position tactical
suite, start-position perft 5 (4,865,609 nodes), formatting, and Clippy passed.
The hybrid depth-8 tree then grew from 513,582 to 556,542 nodes (+8.36%), beyond
the registered 5% ceiling. The equal-scored start move changed from `d2d4/23`
to `b1c3/23`, and the attack position fell from `c3d5/104` to `c3d5/95`.

H5e failed locally, so no remote games ran and the playing code was removed.
