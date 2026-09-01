# Changelog

All notable changes to NEYRANG, formerly NEYRANG, are documented here.

## [Unreleased]

### Added

- The NEYRANG identity and explicit REKHNE search, SANJ evaluation, and SHEGERD
  strength-technique module boundaries, with a fail-closed migration contract
  for binaries, packages, schemas, and NNUE artifacts.
- An OpenBench-compatible root Makefile honoring `EXE=`, plus a contract test that builds the named executable, checks three sequential and three concurrent 180,591-node benches, requires positive parsable NPS, and verifies UCI `Hash`/`Threads` readiness.
- Cross-platform GitHub CI for formatting, Clippy, default/all-feature Rust tests, Python infrastructure tests, match-runner checks, release Perft gates, and the OpenBench contract on Linux and macOS.
- Evidence-first engine-regression and experiment issue forms, a playing-change pull-request template, contribution/security policies, an OpenBench deployment guide, and a source-backed competitive roadmap.
- A reusable paired-match auditor that independently parses PGNs, reconstructs W/D/L and pentanomial counts, verifies opening/color pairs and complete telemetry, audits metadata, and scans strict logs for failure classes.
- A 100,000-position exact-list/order legality oracle covering checks, double checks, pins, en passant, castling, and promotions.
- A test-only immutable pre-G1 SEE oracle, exercised over 100,000 legal positions, 336,083 tactical moves, and 7,393,826 threshold queries.
- A feature-gated `neyrang-sanj-trace-v1` coefficient schema and streaming TSV exporter, verified against production SANJ evaluation over 100,000 legal positions.
- A deterministic pair-first evaluation-corpus builder with provenance manifests, opening-group splits, quiet-position sampling, cross-partition leakage removal, and independently replayable records.
- An independent corpus auditor that reconstructs every sampled position from PGN and rejects content tampering even when manifest hashes are rewritten.
- A deterministic fresh-opening selector with PGN/EPD exclusion sets, canonical-FEN deduplication, fixed hash ranking, provenance output, and fail-closed capacity/overwrite checks.
- Per-engine deterministic node budgets in the paired-match runner, with fail-closed shared/per-engine exclusivity and exact metadata assertions in the independent auditor.
- An explicit immutable-baseline warning policy that retains only a named opponent's exact threefold-PV continuation while rejecting candidate or unrelated warnings; default behavior remains reject-all.
- A frozen fixed-game campaign tool that creates deterministic non-overlapping opening shards, binds workers to external campaign and asset hashes, checksums every result artifact, and independently replays every shard before aggregation.
- A retained root-diversified Lazy-SMP path with worker-private search state, a coherent packed atomic shared TT, exact aggregate node limits, score voting, aggregate UCI telemetry, and an external 1/2/4-thread scaling harness.
- Per-engine thread counts in the fixed paired-match runner, with shared/per-engine exclusivity, range validation, dry-run regression coverage, and explicit metadata.
- Deterministic OpenBench `genfens` support using the complete unsigned 64-bit seed, shard-invariant `seed + index` streams, legal two-ply HCE-filtered opening walks, exact line output, and fail-closed option parsing.
- An independent `python-chess` opening-shard generator/auditor with the OpenBench 15-second stall rule, canonical-position deduplication, executable/source/compiler/license provenance, SHA-256 identities, manifest-first no-clobber EPD publication, and overwrite refusal.
- An isolated fixed-node NNUE self-play recorder with white-relative parent scores, rules-only terminal WDL, score saturation, exact position-restoration checks, and per-reason completion/rejection accounting.
- A partition-first scored-shard wrapper that binds audited opening manifests, keeps color-reversed groups together, monitors generator progress, independently verifies opening membership/legal replay/terminal WDL, and publishes content-addressed Viriformat plus provenance without overwrite.
- Deterministic complete-game NNUE assembly with duplicate-opening and fixed-partition whole-game quarantine, plus parameter-checked paired network diagnostics and deterministic game bootstrap confidence bounds.
- An opt-in opening-stream duplicate quarantine that preserves the registered seed range, retains first canonical occurrences and records every dropped index and digest while default generation remains fail-closed.
- Search-facing NNUE corpus diagnostics for score bias/error/correlation/sign agreement, target loss and deterministic filter exposure, plus explicit fail-closed trainer controls for position filtering and score/result target mixing.
- Final-holdout assembly as a first-class partition, including whole-game train/validation leakage quarantine and hash-verified compatibility with registered historical opening, split and corpus identities.
- Independent NNUE king-bucket exposure analysis over both oriented
  perspectives, used to bound the N2c representation before training.

### Changed

- The development version is now `NEYRANG 0.3.0-dev`; the executable and Rust
  crate are `neyrang`, and the legacy `NEYRANG 0.2.0` tag remains historical
  provenance rather than being rewritten.
- Development legal move generation now avoids make/check/unmake in the common path and validates exceptional moves against simulated final occupancy. The deterministic search tree and checksum are unchanged.
- The retained F1 candidate reduced median depth-8 wall time by 33.99% and raised median NPS by 51.50% across 15 interleaved runs per frozen binary.
- Exact SEE now carries color occupancy and target attackers through the exchange, reveals slider x-rays incrementally, prepares each accepted legal LVA state once, and preserves every prior score, threshold answer, exchange-step count, search tree, and checksum.
- The retained G1 candidate reduced median depth-8 wall time by a further 4.44% and raised median NPS by 4.62% across 15 interleaved runs per frozen binary.
- H0 kept its historical default release byte-for-byte identical to G1 while
  isolating evaluation evidence tooling. The renamed development tree exposes
  that tooling through the non-default `sanj-tools` feature; identity migration
  necessarily changes binary bytes.
- The paired-match auditor now distinguishes nonzero fastchess timeout/crash summary counters from clean zero counters, with regression coverage for both summaries and free-form failures.
- H2d replaces H2c's wall-clock allocation with pre-registered 30,000/29,200-node G1/F1 limits derived from 732,065 plies of rejected-run telemetry; no playing code or evaluation weight changes.
- The fixed match runner records the selected warning policy and refuses to combine the immutable-baseline compatibility policy with fastchess strict mode.
- The match auditor can require the exact registered opening sequence, while distributed metadata carries campaign/shard identities and pair offsets. Sub-millisecond time controls are rejected before games because they cannot round-trip through the PGN header.
- `Threads=1` retains the established local full-key TT and deterministic search entry point; `Threads>1` exercises the retained P3 shared-TT path after passing its binding scaling, deadline, and paired-game decisions.

### Evidence

- F1 completed a strict 2,000-game / 1,000-pair timed screen at 62.20%, with 2,000 normal terminations and no timing, legality, crash, or protocol anomaly. This is development evidence; version `0.2.0` remains the latest release.
- G1 completed a separate strict 2,000-game / 1,000-pair timed screen at 50.98%, with 2,000 normal terminations and no timing, legality, crash, or protocol anomaly. This is a tree-identical performance regression guard; version `0.2.0` remains the latest release.
- H2b completed 4,000 fresh paired games without a timing, crash, legality, warning, or protocol failure and produced 2,910 independently replayed unique evaluation records. The source is retained for later tuning, its holdout remains sealed, and no evaluation weight changed.
- H2c was rejected in full after one timeout following 7,462 complete games; the independently audited prefix is preserved only as failure evidence and is excluded from every corpus and tuning decision.
- P1 completed and independently audited 2,000 cumulative G1-versus-v0.2.0 games at 65.875% (`+114.26 +/-12.79 Elo`), with 2,000 normal terminations, complete telemetry, and zero allowed or rejected warning or other anomaly. This permits a separately registered SPRT but does not change the released version.
- A two-shard real-binary smoke completed 8/8 games and passed independent coordinator replay with exact aggregate W/D/L and pentanomial counts. This validates infrastructure only and is not strength evidence.
- P3 passed its frozen scaling gate at `2.010776x / 3.956388x` aggregate NPS for Threads 2/4. Its binding same-binary 2,000-game Threads-2-versus-Threads-1 screen then completed `711/701/588` (53.075%, `+21.39 +/-10.90 Elo`) with pentanomial `[57,184,437,223,99]`. Independent replay verified all 1,000 pairs and 196,758 plies with no timing, legality, crash, warning, or protocol anomaly. This retains P3 on the development branch but does not change version `0.2.0` or replace a separately preregistered normalized SPRT.
- The first committed N1b recorder campaign completed and independently replayed 3,683/3,683 fixed-node train/validation games with zero rejection and 320,947 white-relative scored positions; 413 partitioned holdout openings were not played. This is data-pipeline evidence only, not training or Elo evidence, and remains below the one-million-position gate.
- N1e completed 160,109/160,109 new train games with zero rejection and assembled 18,140,767 train positions disjoint from the unchanged 463,228-position validation corpus. One exact 16,008,492-position Metal epoch passed both raw/quantized parity gates and lowered fixed-validation MSE from `0.076680656365` to `0.074149893964`; the pre-registered paired game bootstrap interval is wholly negative. This retains the 16M network as offline learning-curve evidence only—there is no engine integration, game result or Elo claim.
- N2a's N1e network lost its strict 1,000-game fixed-node screen `177/633/190` and was rejected for play with no runtime anomaly. N2b then trained a score-only replacement that passed parity and search-facing holdout metrics on a new zero-overlap 22,205-game corpus, but its registered paired bootstrap interval `[-0.000633537, 0.000026689]` crossed zero. It was rejected before games; classical SANJ remains default and neither N2 result is release Elo evidence.

## [0.2.0] - 2026-08-29

### Added

- Legal static exchange evaluation coverage for pins, x-rays, promotions, en passant, and king recaptures.
- Precomputed SEE capture classification with losing captures deferred behind killers and quiets.
- Conservative qsearch pruning of proven losing captures outside check.
- One-ply late move reductions for guarded late quiet moves, with mandatory full-depth re-search.
- Deterministic tactical regression fixtures for mates, captures, promotions, stalemate avoidance, opposition, and quiet defense; every reported PV is replayed for legality.
- Reproducible fixed-size, node-limited, and normalized-SPRT fastchess runners with binary, opening, tool, Git, configuration, PGN, and log identities.

### Changed

- The five-position depth-5 benchmark fell from 448,136 nodes (`a4b453e8ce750456`) to 196,627 nodes (`4a4c31e290740db3`).
- The final candidate accepted the configured H1 in a 786-game normalized SPRT against the immutable `v0.1.0` binary, scoring 61.70% with no recorded protocol failure.
- UCI identity is now `NEYRANG 0.2.0`.

### Rejected

- Guarded null-move pruning was reverted after its 1,000-game capped SPRT against the LMR parent remained inconclusive. Reverse futility pruning was not attempted in this release.

## [0.1.0] - 2026-08-29

- First stable UCI release with legal move generation, reversible state and hashing, classical evaluation, iterative alpha-beta/PVS search, quiescence, transposition table, time management, Perft fixtures, and deterministic benchmarking.
