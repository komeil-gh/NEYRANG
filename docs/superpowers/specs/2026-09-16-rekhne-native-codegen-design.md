# REKHNE H8: host-native code generation

## Trigger evidence

The clean 1,000-game H7 screen scored 23.90% against Blunder 7.6.0. A separate
Stockfish-18 replay of 12,229 NEYRANG decisions showed that mean non-mate move
loss falls from 46.52 centipawns at completed depth five to 17.19 centipawns at
depth seven or higher. Search throughput therefore remains a measured external
weakness.

## Candidate

Keep the complete H7 source, embedded N7 residual, policy, search tree, and UCI
defaults frozen. Build a separate server artifact with Rust
`-C target-cpu=native`; the normal repository build remains portable. This is a
deployment optimization, not a new evaluator or search heuristic.

## Gates

1. The full Rust suite, formatting, and Clippy with warnings denied must pass
   under the native build flags on the remote x86-64 worker.
2. Across 21 interleaved pairs, classical depth 8 and pure-N7 depth 7 must keep
   exact node counts and checksums. Neither path may regress and pure N7 must
   improve median wall time by at least 5%.
3. A clean 1,000-game equal-time match against the frozen H7 binary must have a
   positive point estimate at `0.5+0.005`, 500 reversed opening pairs,
   concurrency 8, Hash 64 MB, and a 200 ms runner margin.
4. A pass opens a separate clean 1,000-game Blunder 7.6.0 screen using the same
   opening set and seed as H7. External progress requires a score above H7's
   frozen 23.90% point estimate. All claims remain specific to this host-native
   artifact.

## Engineering result

Accepted. The native build passed formatting, Clippy with warnings denied, and
the full Rust suite on the registered worker. Across 21 interleaved pairs, the
classical depth-8 benchmark preserved 536,259 nodes and checksum
`9d8d22b14e51010d`; median wall time fell from 361 to 333 ms (8.41%). Pure N7
depth 7 preserved 311,830 nodes and checksum `a03e1698577ee12e`; median wall
time fell from 299 to 239 ms (25.10%).

## Match outcome

The audited parent match completed 1,000 normal games with zero timeout, crash,
forfeit, illegal-move, protocol, or warning anomaly. H8 scored 405 wins, 380
draws, and 215 losses: 59.50%, or `+66.82 +/-17.17 Elo`, with pentanomial
`[25, 79, 159, 155, 82]`.

The audited Blunder match also completed 1,000 normal games with zero anomaly.
H8 scored 141 wins, 266 draws, and 593 losses: 27.40%, or
`-169.27 +/-18.73 Elo`, with pentanomial `[162, 175, 124, 31, 8]`. Against the
same 500 opening pairs, H7 had scored 23.90%. The paired gain is 3.50 percentage
points (95% interval 0.47 to 6.53), equivalent to a +31.92 logistic-Elo change
(paired 95% interval +4.11 to +59.73). The retained binary SHA-256 is
`90be83e5d4cb28320e9f6b128a5c40513f949f0f46edacb26d222e12972d8411`.

H8 is retained as a host-specific deployment artifact. It does not replace the
portable default build and its measured gain must not be transferred to other
CPUs without a fresh benchmark and match check.
