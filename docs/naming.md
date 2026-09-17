# NEYRANG naming and migration contract

**NEYRANG** is the sole project and engine identity. The four names have
distinct technical ownership:

| Name | Meaning | Code and interface ownership |
| --- | --- | --- |
| **NEYRANG** | the chess engine | Cargo package/crate `neyrang`, executable `neyrang`, UCI identity `NEYRANG` |
| **REKHNE** | the strategy it searches with | `src/rekhne`: iterative search, qsearch, TT, limits, time, and parallel coordination |
| **SANJ** | the judgment it evaluates with | `src/sanj`: default handcrafted evaluation, optional exact trace, and feature-gated NNUE judgment |
| **SHEGERD** | techniques that make it stronger | `src/shegerd`: staged move ordering, history heuristics, and SEE |

These boundaries prevent the names from becoming aliases for the same layer.
Chess rules remain in `src/chess`; UCI and developer tools remain protocol and
tool boundaries around the four named concepts.

## Current interface names

- binary and Rust crate: `neyrang`
- UCI name: `NEYRANG 0.3.0`
- evaluation evidence feature: `sanj-tools`
- evaluation evidence command: `neyrang sanj-trace`
- experimental evaluation feature: `nnue`
- experimental UCI option: `EvalFile`
- experimental benchmark: `neyrang bench-nnue`
- current schema prefix: `neyrang-`
- version-1 NNUE magic: the eight bytes `NEYRANG\0`
- NNUE tool packages: `neyrang-nnue-data`, `neyrang-nnue-reference`, and
  `neyrang-nnue-trainer`

The identity contract is intentionally fail-closed. Readers accept only the
documented `NEYRANG\0` artifact format, and pipeline tools accept and emit only
`neyrang-*` schemas. Any pre-contract tensor source must be re-imported into a
current artifact and pass the same parity gates before use.

## Historical evidence boundary

Published tags and recorded hashes remain immutable version evidence, but the
current tracked tree uses only NEYRANG identifiers. Historical binaries or
tensor streams are never treated as current artifacts merely because their
surrounding file was renamed; they must be rebuilt or re-imported through the
current fail-closed contracts.
