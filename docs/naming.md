# NEYRANG naming and migration contract

The project identity changed from **NEYRANG** to **NEYRANG** on the `0.3.0-dev`
line. The four names have distinct technical ownership:

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
- UCI name: `NEYRANG 0.3.0-dev`
- evaluation evidence feature: `sanj-tools`
- evaluation evidence command: `neyrang sanj-trace`
- experimental evaluation feature: `nnue`
- experimental UCI option: `EvalFile`
- experimental benchmark: `neyrang bench-nnue`
- current schema prefix: `neyrang-`
- version-1 NNUE magic: the eight bytes `NEYRANG\0`
- NNUE tool packages: `neyrang-nnue-data`, `neyrang-nnue-reference`, and
  `neyrang-nnue-trainer`

The migration is intentionally fail-closed. Current readers do not silently
accept `NEYRANGNNUE` as the new artifact format, and current pipeline tools emit
`neyrang-*` schemas. A retained old network must be re-imported from its frozen
tensor source into a new artifact and pass the same parity gates before use.

## Historical evidence

Tagged releases through `v0.2.0`, old PGNs, immutable evidence manifests,
recorded hashes, and frozen experiment identities were produced under the NEYRANG
name. They remain historical NEYRANG evidence. Renaming strings inside those files
would invalidate provenance without changing the bytes that were actually
tested.

Documentation may therefore mention NEYRANG only when describing a legacy release,
an immutable artifact, or historical evidence. New commands, packages, schemas,
artifacts, and experiments use NEYRANG and the subsystem names above.
