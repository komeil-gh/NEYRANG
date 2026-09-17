# H30: exact retained-path throughput

## Trigger evidence

The independent `bec082f` baseline scored 27.54% against Blunder 7.6.0, while
profiling and source review found three avoidable costs on the retained path:
active-stage indices were scanned twice, classical SANJ traversed the same
piece sets repeatedly, and SANJ-N7 silently missed the AVX2 output kernel
because its activation quantization is 1536 rather than 255.

H19 and H20 had already preserved registered trees and improved throughput by
2.88% and 3.47%. They were removed after separate 1,000-game equal-time point
estimates of 49.30%, even though both intervals were wide and the patches were
intended to preserve every score and search decision. H30 re-evaluates the
exact code paths as one performance candidate; no playing game has been run
for H30 before this gate was written.

## Hypothesis

Collect active MovePicker indices during their existing classification pass,
fuse the existing classical SANJ piece traversals, and compute the clipped
square output dot product with exact signed 64-bit AVX2 products for validated
activation quantization up to 46340. Do not change weights, move scores,
pruning, reductions, node limits, UCI defaults, or artifact formats.

## Gate

1. The complete quality suite, release Perft set, tactical suite, and UCI smoke
   test must pass on the registered remote x86-64 worker.
2. Parent and candidate must match depth-8 and depth-12 node counts and
   checksums exactly. The current SANJ trace oracle and focused AVX2 scalar
   oracle must pass.
3. Across 31 interleaved native pairs on an otherwise idle worker, candidate
   median throughput must improve by at least 5% without a slower median.
4. A 256-game paired fixed-node same-opening run must have identical aggregate
   W/D/L after color reversal, complete telemetry, and no engine, protocol,
   legality, or timing anomaly.
5. Only after these gates may the candidate replace the independent baseline
   for a fresh equal-resource Blunder screen. That external match measures the
   complete candidate; it does not retroactively turn throughput into an Elo
   claim.

## Outcome

Accepted as an exact performance change. Depth 8 matched the parent at 536,259
nodes with checksum `9d8d22b14e51010d`; depth 12 matched at 29,459,618 nodes
with checksum `0f8416196d8f7941`. Across 31 interleaved depth-8 pairs, median
throughput rose from 1,840,625 to 2,390,342 nodes per second (+29.87%) while
median time fell from 291 ms to 224 ms. The 256-game fixed-node match was
exactly 66 wins, 66 losses, and 124 draws with pentanomial `[0, 0, 128, 0, 0]`;
its independent audit found no anomaly.

The complete quality matrix, Perft 5 (4,865,609 nodes), UCI smoke, and focused
scalar/AVX2 oracle passed on the remote x86-64 worker. In a fresh 512-game
equal-resource Blunder 7.6.0 match, H30 scored 105 wins, 271 losses, and 136
draws: 33.79%, -116.86 +/- 26.59 Elo, and pentanomial
`[72, 70, 76, 28, 10]`. The audit found 512 normal terminations and no candidate
anomaly. This is stronger evidence than the earlier independent baseline's
27.54% point estimate, but H30 still loses decisively to Blunder; no victory or
absolute-rating claim follows from this result.
