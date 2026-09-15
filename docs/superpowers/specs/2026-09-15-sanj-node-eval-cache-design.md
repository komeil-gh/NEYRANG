# SANJ Node Evaluation Reuse

## Hypothesis

REKHNE can ask SANJ for the same static score twice at one node when a pruning
guard does not cut off and a later guard also needs evaluation. Lazily cache
that node-local score and reuse it. Search decisions, node count, checksum, and
UCI behavior must remain identical while depth-8 wall time improves.

## Gate

Build the candidate and frozen `bcfaf7f` parent with identical features and
native CPU flags. Require the registered depth-5 and depth-8 nodes and checksums
to remain identical. Measure at least 15 interleaved depth-8 runs per binary;
reject unless candidate median wall time improves by at least 1% with no test,
protocol, or benchmark anomaly. This is a throughput change, not standalone Elo
evidence.

## Outcome

The candidate preserved the registered depth-5 result exactly at 34,345 nodes
and checksum `b56e1c02181e3033`, and depth 8 at 536,259 nodes and checksum
`9d8d22b14e51010d`. Across 21 interleaved native Windows runs per binary, its
depth-8 median was 309 ms versus the parent's 321 ms, a 3.738% improvement.
H3o passed its throughput gate and is retained without an isolated Elo claim.
