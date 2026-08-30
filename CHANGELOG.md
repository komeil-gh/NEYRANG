# Changelog

All notable changes to NEYRANG are documented here.

## [Unreleased]

### Added

- A reusable paired-match auditor that independently parses PGNs, reconstructs W/D/L and pentanomial counts, verifies opening/color pairs and complete telemetry, audits metadata, and scans strict logs for failure classes.
- A 100,000-position exact-list/order legality oracle covering checks, double checks, pins, en passant, castling, and promotions.

### Changed

- Development legal move generation now avoids make/check/unmake in the common path and validates exceptional moves against simulated final occupancy. The deterministic search tree and checksum are unchanged.
- The retained F1 candidate reduced median depth-8 wall time by 33.99% and raised median NPS by 51.50% across 15 interleaved runs per frozen binary.

### Evidence

- F1 completed a strict 2,000-game / 1,000-pair timed screen at 62.20%, with 2,000 normal terminations and no timing, legality, crash, or protocol anomaly. This is development evidence; version `0.2.0` remains the latest release.

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
