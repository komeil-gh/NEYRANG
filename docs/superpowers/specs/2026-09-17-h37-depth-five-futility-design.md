# H37: depth-five guarded forward futility

## Trigger evidence

H35's comparable external score is 33.30% against Blunder 7.6.0. H36 showed
that a candidate can win both NEYRANG parent screens and still regress against
the external opponent, so external comparison remains mandatory. The retained
forward-futility rule stops at depth four, while the same rule already protects
the first move, checks, captures, promotions, preferred moves, killers, PV and
root nodes, mate windows, low-material endings, and synthetic null subtrees.

## Candidate

Extend only the maximum eligible forward-futility depth from four to five.
Keep the existing `100 * depth` margin and every safety guard unchanged. Search
ordering, evaluation, artifacts, other pruning rules, and UCI defaults remain
frozen on H35.

## Gates

1. The focused regression must exercise depth-five pruning and preserve the
   exact position after search. Existing tactical, terminal, and search-state
   tests remain authoritative.
2. Formatting, clippy, the complete all-features Rust suite, Perft 5, UCI
   smoke, and deterministic benchmarks must pass on the SSH worker.
3. A clean 512-game paired 50,000-node screen against frozen H35 must score at
   least 50% with no candidate anomaly.
4. A passing candidate receives a 512-game `0.5+0.005` parent screen on the
   same pairs; its point estimate must be non-negative.
5. Only both parent passes open the comparable UHO/`0.5+0.005` Blunder 7.6.0
   screen. Retention requires exceeding H35's 33.30% point estimate without a
   NEYRANG engine, protocol, timing, or opening-pair anomaly.

## Outcome

Rejected and reverted. Formatting, clippy, the complete all-features Rust
suite, Perft 5, and UCI tests passed. The depth-8 tree moved only from 536,259
to 534,157 nodes (-0.39%). The clean fixed-node parent screen passed with 152
wins, 214 draws, and 146 losses: **50.59%**, `+4.07 +/-20.88 Elo`,
pentanomial `[21, 40, 124, 54, 17]`.

The clean equal-time parent screen then scored 160 wins, 188 draws, and 164
losses: **49.61%**, `-2.71 +/-22.44 Elo`, pentanomial
`[21, 58, 104, 50, 23]`. Both audits accepted all 512 games with no anomaly,
but the negative equal-time point estimate failed gate 4. No Blunder screen
was opened; the depth limit and focused boundary fixture were restored to H35.
