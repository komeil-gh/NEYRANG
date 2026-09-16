# SANJ H7: AVX2 activation

## Trigger evidence

A fresh 256-game baseline against Blunder 7.6.0 was replayed into 12,229
NEYRANG decisions and labeled by Stockfish 18 at 20,000 nodes per search. After
excluding mate-score sentinels, mean teacher loss fell from 74 cp at reported
depth four or less and 46.52 cp at depth five to 17.19 cp at depth seven or
more. The same match reported median NPS of 737,025 for NEYRANG and 1,522,289
for Blunder. This made score-preserving throughput the next bounded target.

An earlier threshold-SEE ordering candidate was repeated only as a local gate
and rejected before games: its depth-6 tree grew from 88,691 to 91,487 nodes
(+3.15%), while the three-run median rose from 58 to 68 ms (+17.24%). The
accepted search and exact tactical ordering were restored before H7.

## Hypothesis

The retained 10% N7 residual evaluates both classical SANJ and the incremental
network. On the registered remote x86-64 worker, pure N7 was materially slower
than classical SANJ. Vectorize the 128-wide clipped-square output dot product
without changing any score, search decision, artifact, or UCI option.

The candidate is runtime-dispatched only when AVX2 is available. Other targets
retain the scalar implementation. The version-4 latent-imbalance artifact also
retains the scalar path because its third absolute-difference channel is not
part of the retained N7 network.

## Gate

Require the complete Rust suite, formatting, and clippy with warnings denied to
pass on the remote worker. The N7 depth-7 benchmark must preserve nodes and
checksum exactly across 15 interleaved parent/candidate pairs and improve median
wall time by at least 5%. A fixed-node end-to-end UCI match must preserve every
reported node/depth distribution and paired result while improving median NPS.

## Outcome

Accepted. All Rust targets passed and clippy completed with warnings denied.
Every depth-7 run produced 311,830 nodes and checksum `a03e1698577ee12e`.
The parent's 15-run median was 296 ms; the AVX2 candidate's median was 259 ms,
a 12.5% improvement.

The 16-game, 10,000-node, concurrency-1 UCI check produced eight mirrored
result pairs and identical node, depth, seldepth, and result distributions.
Across 671 reported decisions per engine, median NPS increased from 863,790 to
942,506 (+9.11%); mean NPS increased from 904,997 to 989,154 (+9.30%). The
candidate changes throughput only, so this fixed-node check makes no Elo claim.

A separate equal-time confirmation used 1,000 games at `0.5+0.005`, 500
color-reversed opening pairs, and concurrency 8 on the same remote worker. The
candidate scored 329 wins, 404 draws, and 267 losses: 53.10%, or +21.57 logistic
Elo against its parent. The paired-outcome normal approximation gives a 95%
interval of +6.05 to +37.17 Elo. Its pentanomial result was
`[18, 121, 198, 107, 56]`. Independent PGN replay accepted all 1,000 games,
found 500 unique paired openings, normal termination for every game, balanced
colors, complete telemetry, and no negative time-left sample.

The external Blunder confirmation used the same 500 opening pairs and seed for
separate 1,000-game parent and candidate matches at `0.5+0.005`, concurrency 8,
and a 200 ms runner time margin. The parent scored 24.50%; the candidate scored
23.90%. Comparing the two scores opening-pair by opening-pair gives a -0.60
percentage-point estimate, or -5.68 logistic Elo, with a paired 95% interval of
-34.37 to +23.01 Elo. Both retained matches had normal termination for all
games and zero timeout, crash, forfeit, illegal-move, or protocol anomalies.
H7 remains a semantics-preserving throughput improvement, but this external
screen does not establish a strength gain against Blunder.
