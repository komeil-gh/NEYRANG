# Changelog

All notable changes to NEYRANG are documented here.

## [Unreleased]

### Added

- A reusable paired-match auditor that independently parses PGNs, reconstructs W/D/L and pentanomial counts, verifies opening/color pairs and complete telemetry, audits metadata, and scans strict logs for failure classes.
- A 100,000-position exact-list/order legality oracle covering checks, double checks, pins, en passant, castling, and promotions.
- A test-only immutable pre-G1 SEE oracle, exercised over 100,000 legal positions, 336,083 tactical moves, and 7,393,826 threshold queries.
- A feature-gated `neyrang-eval-trace-v1` coefficient schema and streaming TSV exporter, verified against production evaluation over 100,000 legal positions.
- A deterministic pair-first evaluation-corpus builder with provenance manifests, opening-group splits, quiet-position sampling, cross-partition leakage removal, and independently replayable records.
- An independent corpus auditor that reconstructs every sampled position from PGN and rejects content tampering even when manifest hashes are rewritten.
- A deterministic fresh-opening selector with PGN/EPD exclusion sets, canonical-FEN deduplication, fixed hash ranking, provenance output, and fail-closed capacity/overwrite checks.
- Per-engine deterministic node budgets in the paired-match runner, with fail-closed shared/per-engine exclusivity and exact metadata assertions in the independent auditor.
- An explicit immutable-baseline warning policy that retains only a named opponent's exact threefold-PV continuation while rejecting candidate or unrelated warnings; default behavior remains reject-all.
- A frozen fixed-game campaign tool that creates deterministic non-overlapping opening shards, binds workers to external campaign and asset hashes, checksums every result artifact, and independently replays every shard before aggregation.
- A pre-registered root-diversified Lazy-SMP candidate with worker-private search state, a coherent packed atomic shared TT, exact aggregate node limits, score voting, aggregate UCI telemetry, and an external 1/2/4-thread scaling harness.
- Per-engine thread counts in the fixed paired-match runner, with shared/per-engine exclusivity, range validation, dry-run regression coverage, and explicit metadata.

### Changed

- Development legal move generation now avoids make/check/unmake in the common path and validates exceptional moves against simulated final occupancy. The deterministic search tree and checksum are unchanged.
- The retained F1 candidate reduced median depth-8 wall time by 33.99% and raised median NPS by 51.50% across 15 interleaved runs per frozen binary.
- Exact SEE now carries color occupancy and target attackers through the exchange, reveals slider x-rays incrementally, prepares each accepted legal LVA state once, and preserves every prior score, threshold answer, exchange-step count, search tree, and checksum.
- The retained G1 candidate reduced median depth-8 wall time by a further 4.44% and raised median NPS by 4.62% across 15 interleaved runs per frozen binary.
- H0 keeps the default release byte-for-byte identical to G1 while isolating evaluation evidence tooling behind the non-default `eval-tools` feature.
- The paired-match auditor now distinguishes nonzero fastchess timeout/crash summary counters from clean zero counters, with regression coverage for both summaries and free-form failures.
- H2d replaces H2c's wall-clock allocation with pre-registered 30,000/29,200-node G1/F1 limits derived from 732,065 plies of rejected-run telemetry; no playing code or evaluation weight changes.
- The fixed match runner records the selected warning policy and refuses to combine the immutable-baseline compatibility policy with fastchess strict mode.
- The match auditor can require the exact registered opening sequence, while distributed metadata carries campaign/shard identities and pair offsets. Sub-millisecond time controls are rejected before games because they cannot round-trip through the PGN header.
- `Threads=1` retains the established local full-key TT and deterministic search entry point; `Threads>1` now exercises the isolated P3 shared-TT path on the development branch pending its binding scaling and paired-game decisions.

### Evidence

- F1 completed a strict 2,000-game / 1,000-pair timed screen at 62.20%, with 2,000 normal terminations and no timing, legality, crash, or protocol anomaly. This is development evidence; version `0.2.0` remains the latest release.
- G1 completed a separate strict 2,000-game / 1,000-pair timed screen at 50.98%, with 2,000 normal terminations and no timing, legality, crash, or protocol anomaly. This is a tree-identical performance regression guard; version `0.2.0` remains the latest release.
- H2b completed 4,000 fresh paired games without a timing, crash, legality, warning, or protocol failure and produced 2,910 independently replayed unique evaluation records. The source is retained for later tuning, its holdout remains sealed, and no evaluation weight changed.
- H2c was rejected in full after one timeout following 7,462 complete games; the independently audited prefix is preserved only as failure evidence and is excluded from every corpus and tuning decision.
- P1 completed and independently audited 2,000 cumulative G1-versus-v0.2.0 games at 65.875% (`+114.26 +/-12.79 Elo`), with 2,000 normal terminations, complete telemetry, and zero allowed or rejected warning or other anomaly. This permits a separately registered SPRT but does not change the released version.
- A two-shard real-binary smoke completed 8/8 games and passed independent coordinator replay with exact aggregate W/D/L and pentanomial counts. This validates infrastructure only and is not strength evidence.

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
