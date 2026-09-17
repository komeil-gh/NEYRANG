# H38: supported knight activity

## Trigger evidence

The comparable H35 baseline still scores only 33.30% against Blunder 7.6.0.
H36 and H37 show that broader shallow pruning can improve one internal metric
without improving the external opponent result. SANJ currently values a knight
only by material, board geometry, and legal mobility; it cannot distinguish an
advanced knight anchored by a pawn on a square that enemy pawns can no longer
challenge.

## Candidate

Add three virtual knight-activity units (15 middlegame centipawns through the
existing mobility weight) for a knight that is:

- on files c through f and relative ranks four through six;
- defended by at least one friendly pawn; and
- not challengeable by any enemy pawn remaining ahead on an adjacent file.

This is deliberately an activity refinement rather than a new free-standing
table or imported weight. Endgame score, material, PSQT, search, ordering,
artifacts, and UCI defaults remain frozen on H35. The independent SANJ trace
must reconstruct the same virtual activity exactly without changing its schema.

## Gates

1. Focused tests must cover supported, unsupported, challengeable, wing, and
   colour-mirrored cases. The 100,000-position production/trace oracle must pass.
2. Formatting, clippy, the complete all-features Rust suite, Perft 5, UCI
   smoke, and deterministic benchmarks must pass on the SSH worker.
3. A clean 512-game paired 50,000-node screen against frozen H35 must score at
   least 50% with no candidate anomaly.
4. A passing candidate receives a 512-game `0.5+0.005` parent screen; its point
   estimate must be non-negative.
5. Only both parent passes open the comparable UHO/`0.5+0.005` Blunder 7.6.0
   screen. Retention requires exceeding H35's 33.30% point estimate without a
   NEYRANG engine, protocol, timing, or opening-pair anomaly.

## Outcome

Rejected and reverted.

- All quality checks passed, including the 100,000-position trace oracle,
  Perft 5 at 4,865,609 nodes, and deterministic depth-8 bench at 538,113
  nodes with checksum `32c7dc82f55537f9`.
- The paired 50,000-node parent screen passed only narrowly: 153 wins, 213
  draws, and 146 losses in 512 games, or 50.68% (`+4.75 +/- 20.58 Elo`).
- The decisive equal-time parent screen failed: 150 wins, 205 draws, and 157
  losses in 512 games, or 49.32% (`-4.75 +/- 23.11 Elo`). Both engines used
  one thread, 64 MiB hash, `0.5+0.005`, and the same colour-reversed opening
  pairs; the strict audit passed.

The Blunder gate was not opened. This candidate is direct evidence that a
positive node-limited point estimate is insufficient when the equal-time
result is negative.
