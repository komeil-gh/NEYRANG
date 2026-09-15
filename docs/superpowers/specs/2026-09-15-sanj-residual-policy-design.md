# SANJ Residual and SHEGERD Policy Design

## Objective

Improve NEYRANG's playing strength without making the tournament binary depend
on a GPU. Training may use the RTX 3080 Ti worker, but inference must remain a
portable, one-thread CPU workload and classical SANJ must remain the safe
default.

The immediate target is not an absolute Elo claim. A candidate earns promotion
only by passing a small paired screen, then the frozen S1 gate, and finally a
separate Blunder match.

## Chosen shape

The first candidate reuses the existing SHEGERD-P1 corpus, fitter, quantized
artifact, and fixed-capacity MovePicker. It adds a fail-closed policy artifact
loader and an optional additive move-ordering score. TT/PV moves, SEE tactical
classes, killers, and legal-move completeness remain authoritative. Policy can
only rank moves inside an existing stage, so a poor artifact cannot turn a
losing capture into a good capture or bypass the established search guards.

Classical SANJ remains unchanged for this candidate. A later evaluator candidate
may learn a bounded residual relative to classical SANJ, but only after the
policy path demonstrates that offline metrics predict game results. Replacing
SANJ outright or requiring GPU inference is out of scope.

## Runtime contract

- The normal build remains dependency-free and classical.
- The policy path is compiled behind a non-default feature and selected through
  an explicit UCI artifact option.
- Loading rejects bad magic, version, dimensions, length, or checksum before
  search state changes.
- Each score is a small sum of quantized table entries. No allocation, file I/O,
  synchronization, or floating-point work occurs in the node path.
- The policy contribution is bounded below killer priority and below the gap
  between good and losing tactical stages.

## Features

The version-1 policy uses the already registered six additive feature families:

1. side-normalized from/to squares;
2. mover and normalized destination;
3. victim and promotion;
4. material phase;
5. previous-move destination and current destination;
6. SEE bucket for tactical moves.

The runtime must reproduce the trainer's exact orientation, phase thresholds,
piece codes, offsets, and quantization. Tests compare known fixtures against an
independent expected score.

## Search integration

REKHNE owns the previous-move context and passes it to the MovePicker. SHEGERD
adds policy scores only when a stage is initialized:

- good and losing tactical moves keep their SEE classification and receive a
  bounded policy tie-break;
- killers keep their fixed priority and receive no policy boost capable of
  changing that priority;
- quiet moves combine butterfly history, castling preference, and bounded
  policy score;
- quiescence preserves the existing tactical-only behavior.

No new pruning rule is introduced. The deterministic tree is expected to change
only when a policy artifact is explicitly loaded.

## Evidence sequence

1. Run format, loader, scorer, MovePicker, UCI, perft, and full regression tests.
2. Verify that a build with no policy selected has the registered classical
   benchmark tree and checksum.
3. Fit and independently audit one policy artifact from the existing unsealed
   training/validation partitions; do not inspect or train on a sealed holdout.
4. Run 64 color-reversed opening pairs at fixed nodes with eight concurrent
   games pinned across the eight physical cores. Reject immediately on a
   negative point estimate, anomaly, or NPS regression beyond the registered
   ceiling.
5. If accepted, run the frozen S1 gate under the same engine resources. Only an
   S1 pass permits a separate Blunder match.

Parallelism changes evidence transport, not engine resources: every engine uses
one thread, ponder is off, openings are paired, and both sides receive identical
limits. Eight concurrent games are the screening maximum for the eight physical
cores; larger values are not used for promotion evidence.

## Failure and rollback

An absent or rejected artifact leaves classical SANJ and existing ordering
unchanged. A loader error is reported before search and does not partially
activate policy state. A failed game gate rejects the artifact and feature
activation; it does not trigger more training on the same labels. The next
experiment must change the hypothesis, data, or representation and register a
new gate.
