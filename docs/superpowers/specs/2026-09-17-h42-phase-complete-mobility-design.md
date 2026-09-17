# H42: phase-complete mobility

## Trigger evidence

The retained H41 build scores 35.74% against Blunder 7.6.0 on the registered
256 opening pairs. H38 showed that one isolated outpost bonus is not enough.
SANJ's broader structural gap is simpler: legal mobility contributes only to
the middlegame, so the term disappears exactly when open boards make long-range
piece activity most relevant.

## Candidate

Reuse the attack sets and mobility counts already computed by SANJ, adding
conservative endgame weights of 3/4/6/2 centipawns for knight, bishop, rook,
and queen mobility. No new board traversal, table, dependency, network, search
rule, or UCI option is allowed. The exact trace advances to schema v3 because
the same four raw coefficients now reconstruct both tapered phases; the
registered 42 fitted weights stay unchanged and the four new endgame constants
remain fixed.

## Gates

1. The 100,000-position production/trace oracle, formatter, clippy, complete
   all-features suite, Perft 5, UCI smoke, and deterministic benchmarks pass.
2. A clean 512-game paired 50,000-node parent screen against frozen H41 scores
   at least 50% without a candidate anomaly.
3. A passing candidate receives a clean 512-game paired `0.5+0.005` parent
   screen; its point estimate must be non-negative.
4. Only both parent passes open the exact-pair Blunder 7.6.0 screen. Retention
   requires exceeding H41's 35.74% point estimate without a candidate anomaly.

## Outcome

Rejected and reverted.

All correctness gates passed, including the 100,000-position production/trace
oracle, the complete Rust and Python suites, Perft 5 at 4,865,609 nodes, and
clean UCI smoke. The deterministic depth-8 benchmark changed to 484,524 nodes
with checksum `cf582c0efec670ab`; depth 12 changed to 26,261,583 nodes with
checksum `e85c112c099ff0ea`.

The 512-game fixed-node parent gate appeared positive: 155 wins, 213 draws,
and 144 losses, or 51.07% (`+7.47 +/- 22.56 Elo`). The decisive equal-time
gate then failed by a wide margin: 117 wins, 188 draws, and 207 losses, or
41.21% (`-61.71 +/- 22.18 Elo`). Both audits parsed all games, all 256 reversed
opening pairs, normal terminations, and zero crash, timeout, illegal-move,
protocol, or warning anomalies. The candidate therefore received no external
Blunder games. This is further evidence that a smaller or positive fixed-node
tree is not sufficient when the evaluation change makes equal-time play worse.
