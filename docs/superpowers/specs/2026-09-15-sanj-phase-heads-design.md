# SANJ H5f: material-routed output heads

## Hypothesis

The retained N7 network forces opening, middlegame, and endgame positions
through one 256-to-1 output projection. Its three king-relative input banks can
represent king context, but one shared head still has to fit materially distinct
score regimes. Four output heads selected only by remaining piece count add
2,307 parameters and one constant-time index without widening the incremental
feature transformer.

The bucket rule is the pinned Bullet `MaterialCount<4>` contract:
`(piece_count - 2) / 8`. It maps 2-9, 10-17, 18-25, and 26-32 pieces to heads
0-3. Bullet's official output-bucket example uses this routing, and current
Stockfish NNUE likewise maintains multiple layer stacks rather than one output
path for every material phase:

- <https://github.com/jw1912/bullet/blob/main/examples/progression/2_output_buckets.rs>
- <https://github.com/official-stockfish/Stockfish/blob/master/src/nnue/nnue_architecture.h>

## Candidate

Register artifact version 3 and feature-set id 3 as
`Chess768x3hm4ph`: the existing factorised three-bank input transformer,
SCReLU-128 dual-perspective hidden state, and four material-routed scalar heads.
Versions 1 and 2 remain byte-compatible. Quantised output weights are stored as
four contiguous 256-value rows, followed by four `i32` biases. Incremental
accumulators track piece count; captures decrement it and king refreshes rebuild
it from occupancy.

No new corpus, dependency, search heuristic, feature width, EvalMix default, or
UCI option is introduced.

## Gates

1. Runtime/reference decoding, raw export transposition, material routing,
   quantisation parity, all tests, formatting, and Clippy must pass.
2. Train exactly one four-head candidate from the approved N6 corpus with the
   frozen N7 schedule and seed. No N9 root, partial game, or target may be reused.
3. A fresh root-disjoint validation screen must improve quantised MSE over N7
   and pass the existing parity ceilings. Failure ends H5f before engine games.
4. A passing offline candidate faces H4m in independent fixed-node and equal-time
   parent screens. Both require non-negative point estimates and zero anomalies.
5. Only both passing parent screens open a fresh Blunder 7.6.0 screen; retention
   requires exceeding H4m's 26.56% point estimate. H4m remains untouched until
   every gate passes.

