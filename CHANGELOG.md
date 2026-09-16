# Changelog

All notable changes to NEYRANG are documented here.

## [Unreleased]

### Added

- A default-enabled Stockfish-compatible SANJ evaluator using `nnue-rs 0.4.0`,
  with fail-closed external loading and the official Stockfish 18 small network
  embedded for reproducible release builds.
- Experimental version-4 `Chess768x3hmli` NNUE artifacts and trainer/reference
  support for a low-cost post-SCReLU absolute dual-perspective imbalance channel.
- A fail-closed CPU SHEGERD policy path that loads checksum-bound
  version-2 additive tables and ranks moves only inside existing MovePicker
  stages, plus separately hashed `PolicyFile` match-runner support.
- Policy fitting and independent audit exclude cross-stage comparisons that the
  runtime MovePicker cannot reorder.
- An integer `EvalMix` bounds NNUE output as a residual around classical SANJ.
- Exact-quota teacher corpus selection with registered-order transposition
  deduplication, input/output hash binding, and streamed exposure expansion.
- Exclusive-create protocol evidence logging with expected-byte SHA-256 checks
  and persistent write-error detection, outside the playing engine.
- A no-clobber fixed-partition SANJ corpus mode for separately sourced final
  holdouts, with independent PGN replay that verifies every record remains in
  the registered sealed partition while legacy 80/10/10 builds stay unchanged.
- Concurrency-safe SANJ corpus pairing in both builder and independent auditor,
  reconstructing color-reversed games by numeric `Round` identity instead of
  assuming completion-order adjacency in retained PGNs.
- An external macOS search-observability harness that runs an unchanged frozen
  profiling binary, hash-binds raw `sample` output and deterministic bench
  results, publishes a stable no-clobber category schema, and cleans up its own
  benchmark process on sampler failure.
- The NEYRANG identity and explicit REKHNE search, SANJ evaluation, and SHEGERD
  strength-technique module boundaries, with a fail-closed migration contract
  for binaries, packages, schemas, and NNUE artifacts.
- An OpenBench-compatible root Makefile honoring `EXE=`, plus a contract test that builds the named executable, checks three sequential and three concurrent 180,591-node benches, requires positive parsable NPS, and verifies UCI `Hash`/`Threads` readiness.
- Cross-platform GitHub CI for formatting, Clippy, default/all-feature Rust tests, Python infrastructure tests, match-runner checks, release Perft gates, and the OpenBench contract on Linux and macOS.
- Evidence-first engine-regression and experiment issue forms, a playing-change pull-request template, contribution/security policies, an OpenBench deployment guide, and a source-backed competitive roadmap.
- A reusable paired-match auditor that independently parses PGNs, reconstructs W/D/L and pentanomial counts, verifies opening/color pairs and complete telemetry, audits metadata, and scans strict logs for failure classes.
- A 100,000-position exact-list/order legality oracle covering checks, double checks, pins, en passant, castling, and promotions.
- A test-only immutable pre-G1 SEE oracle, exercised over 100,000 legal positions, 336,083 tactical moves, and 7,393,826 threshold queries.
- A feature-gated `neyrang-sanj-trace-v2` coefficient schema and streaming TSV exporter, including bounded king danger and verified against production SANJ evaluation over 100,000 legal positions.
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
- Version-2 `Chess768x3hm` artifact, scalar reference, SANJ inference and
  factorised trainer support, including king-move accumulator refresh and a
  no-clobber merged-float exporter for Bullet factoriser checkpoints.

### Changed

- The local and CI quality gates now run every Rust tool with committed lockfiles,
  cover the portable trainer and policy-trace exporter, and leave generated
  private runtime artifacts ignored. CI installs the complete pinned Python
  test dependencies rather than the runtime-only subset, pins the Rust toolchain,
  and pins the checkout action to its reviewed release commit.
- Historical machine-specific executables were removed from the tracked tree;
  their source commits remain available in Git history, and a repository
  contract test prevents generated `builds/` artifacts from returning.
- Stockfish-compatible SANJ evaluation now advances preallocated per-ply
  accumulators instead of refreshing the full network at every static
  evaluation; a full-refresh parity test protects the score contract.
- The Stockfish-compatible board adapter now enumerates occupied squares
  directly from NEYRANG's twelve piece bitboards instead of probing all 64
  mailbox squares on every NNUE board traversal.
- The separately trained SHEGERD move policy remains bundled and loadable with
  `PolicyFile=<embedded>`, but the default is now `<empty>` so the retained
  Stockfish-compatible evaluator uses the engine's native stage ordering.
- The embedded SHEGERD policy is now P3: a conservative 75% P2 / 25%
  Stockfish-18 teacher blend. It scored 51.86% (`+12.92 +/-13.99 Elo`,
  LOS 96.51%) in a 1,372-game disjoint fixed-node confirmation and improved
  the same-opening Blunder score from 28.42% to 33.79% over 512 games per
  engine. Both retained matches completed without engine or protocol failure.
- Default release builds now embed the accepted Stockfish 18 small network at
  `EvalMix=100` and the teacher-residual SHEGERD-P3 policy. `<empty>` still
  selects the classical evaluator or ordering, and `--no-default-features`
  keeps an explicit research-only classical build available. Native NEYRANG
  NNUE artifacts remain supported for controlled experiments.
- H5h's latent-imbalance network passed format/runtime parity and improved
  fresh Stockfish-18 WDL-space MSE by 1.51%, but its paired-bootstrap 95% upper
  bound remained positive. A later 256-game fixed-node check scored 48.44%
  against N7, confirming its rejection.
- Teacher shard export now creates a missing parent directory for its exclusive
  output directory, preventing audited multi-shard runs from failing before the
  first manifest is written.
- The tracked source, documentation, examples, package names, executable, UCI
  identity, and current data contracts now use NEYRANG exclusively. A repository
  contract test rejects any tracked path or byte sequence containing the retired
  identity, and current readers reject pre-contract schemas rather than silently
  relabeling historical artifacts.
- The development version is `NEYRANG 0.3.0-dev`; the executable and Rust crate
  are `neyrang`, while the immutable `v0.2.0` tag remains version provenance.
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
- Expected-opening audit canonicalization now removes uncapturable en-passant
  fields consistently with fastchess, while still rejecting a legally
  capturable en-passant mismatch.
- `Threads=1` retains the established local full-key TT and deterministic search entry point; `Threads>1` exercises the retained P3 shared-TT path after passing its binding scaling, deadline, and paired-game decisions.
- H3e adds guarded reverse-futility pruning only at non-root, non-PV depths one through three, preserving legal-terminal, check, mate-window, null-subtree, TT-quiet-move, and low-material safeguards.
- H3f adds shallow late-move pruning after a depth-scaled searched-move floor while preserving preferred moves, killers, tactical moves, nodes in check, PV/root nodes, and mate safety.
- H3g adds a check-aware shallow parent futility gate with a `100 * depth` SANJ margin and conservative move, material, mate-window, PV/root, and null-subtree guards.
- H3h reuses the staged MovePicker's exact SEE result to prune sufficiently losing late captures through depth six without a second SEE call, while preserving first/preferred moves, checks, promotions, PV/root nodes, null subtrees, and mate defense.
- H3l adds a SANJ-specific bounded king-ring term that rewards coordinated pressure rather than lone-piece gestures. It requires multiple non-pawn attackers, or queen backing for one attacker, and caps middlegame danger at 120 centipawns.
- H3o reuses one lazily computed static SANJ score across pruning guards at the
  same node without changing their conditions or outcomes.
- H8 retains a separately benchmarked host-native deployment artifact while
  leaving the portable default build and all search decisions unchanged.
- H14 adds a BMI2/PEXT sliding-attack table only to host-native x86-64 builds;
  the portable ray backend remains the fallback and independent oracle.
- H15 keeps H14 source-identical and adds profile-guided code layout only to a
  separately tested host-native artifact; portable release builds are unchanged.
- H18 keeps each lazy MovePicker stage in one fixed-capacity active-index range,
  so repeated best-move selection no longer rescans unrelated or already used
  moves. Scores, stage boundaries, tie order, tree, and checksum are unchanged.
### Evidence

- O1 captured 6,326 macOS top-of-stack samples from an unchanged SHA-256-bound
  profiling binary while preserving the registered depth-11 121,149,567-node
  tree and checksum. Classical SANJ ranked first at 39.30%; this prioritizes a
  separate experiment and is not NPS, Elo, playing-strength, or release proof.
- The first 2,000-game H3a data attempt was rejected because required
  concurrency fixes changed registered tool identities. H3a-R1 then completed
  2,000 replacement games from 1,000 different openings with zero H2b/H2d or
  rejected-attempt overlap. Independent match and corpus audits accepted a
  sealed 5,440-record final holdout from 825 sampled pairs; its match aggregate,
  rows, targets, feature values, predictions and loss remain uninspected. No
  evaluator weight or playing source changed.
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
- N2c trained the single registered three-bank king-relative candidate. It
  passed merged raw/quantized and engine/reference parity, improved selection
  MAE and produced a wholly negative paired-loss interval, but its search-score
  sign agreement `0.904318802` missed the frozen `0.904808319` floor. The
  candidate was rejected before holdout generation with zero games and no Elo
  claim.
- H3e reduced the deterministic depth-8 tree by 53.60% and passed two independently audited 1,000-game paired UHO gates: `+32.76 +/-15.95 Elo` at 20,000 fixed nodes and `+49.32 +/-16.85 Elo` at `0.5+0.005`, with 2,000 normal terminations and no timing, legality, crash, warning, or protocol anomaly.
- H3f reduced the retained depth-8 tree by a further 59.89% and passed two independently audited 1,000-game paired UHO gates: `+97.69 +/-17.84 Elo` at 20,000 fixed nodes and `+41.19 +/-16.93 Elo` at `0.5+0.005`, again with 2,000 normal terminations and no timing, legality, crash, warning, or protocol anomaly.
- H3g reduced the retained depth-8 tree by 5.85%; its independently audited 1,000-game gates scored `+21.92 +/-17.00 Elo` at 20,000 nodes and `+4.86 +/-16.01 Elo` at `0.5+0.005`, with 2,000 normal terminations and no timing, legality, crash, warning, or protocol anomaly. The equal-time gain is not statistically proven.
- H3h reduced the retained depth-8 tree by 12.34%; its independently audited 1,000-game gates scored `+13.21 +/-16.17 Elo` at 20,000 nodes and `+6.25 +/-17.43 Elo` at `0.5+0.005`, with 2,000 normal terminations and no timing, legality, crash, warning, or protocol anomaly. Neither gain is statistically proven.
- H3l reduced the retained depth-8 tree from 567,113 to 536,259 nodes while its five-run local median changed from 275 to 292 ms. Its independently audited 1,000-game gates scored `+2.43 +/-16.98 Elo` at 20,000 nodes and `+2.78 +/-18.26 Elo` at `0.5+0.005`, with positive point estimates and no audit error. Neither gain is statistically proven.
- H3o preserved the 34,345-node depth-5 and 536,259-node depth-8 trees and both
  checksums exactly. Its 21-run interleaved native Windows depth-8 median was
  309 ms versus 321 ms for the frozen parent, a 3.738% improvement. This is
  retained throughput evidence, not a standalone Elo claim.
- H8 preserved exact classical and pure-N7 benchmark identities while reducing
  their 21-pair medians by 8.41% and 25.10%. It beat portable H7 in 1,000 clean
  equal-time games at 59.50% (`+66.82 +/-17.17 Elo`) and scored 27.40% against
  Blunder 7.6.0, versus H7's same-opening 23.90%. The paired external gain was
  +3.50 percentage points (95% interval +0.47 to +6.53), or +31.92 logistic Elo
  (paired 95% interval +4.11 to +59.73). This claim is specific to the tested
  host-native binary.
- H9 reuses each classical non-pawn attack set across SANJ mobility and king
  pressure without changing a score. It preserved the 536,259-node depth-8
  tree and checksum, reduced the 21-pair native median from 319 ms to 300 ms,
  and beat H8 `331/370/299` (51.60%, `+11.12 +/-16.20 Elo`) in 1,000 clean
  equal-time games. Its clean same-opening Blunder screen scored 28.10% versus
  H8's 27.40%; the paired +0.70-point interval (-2.50 to +3.90) crosses zero,
  so the external gain is directional rather than statistically proven.
- H12 bypasses recursive SEE when the post-move destination has no geometric
  enemy recapture. It preserved the exact H9 tree and checksum while reducing
  the 21-pair native median from 330 ms to 313 ms (5.43%). Its audited
  1,000-game parent match scored 50.45% (`+3.13 +/-16.76 Elo`), and its
  same-opening Blunder score rose from 28.10% to 29.75%. The paired +1.65-point
  interval (-1.70 to +5.10) crosses zero, so the external gain remains
  directional rather than statistically proven.
- H14 preserved the exact H12 depth-8 tree and checksum while reducing the
  21-pair native median from 289 ms to 257 ms (12.45%). It beat H12
  `334/374/292` in 1,000 audited equal-time games (52.10%,
  `+14.60 +/-16.02 Elo`) and scored 30.00% against Blunder versus H12's
  same-opening 29.75%; the +0.25-point external difference is directional only.
- H15 preserved the exact H14 depth-8 tree/checksum and Perft 5 while reducing
  the 31-pair native median from 259 ms to 251 ms (3.23% paired median
  throughput). It scored `342/359/299` against H14 in 1,000 audited equal-time
  games (52.15%, `+14.95 +/-16.63 Elo`). On the identical 500 opening/color
  pairs against Blunder it scored 31.85% versus H14's 30.00%; the paired
  +1.85-point interval (-1.20 to +4.95) crosses zero, so the external gain is
  directional rather than statistically proven.
- H18 preserved the exact H14 tree/checksum and improved clean 31-pair native
  throughput by 4.49%. Its independently audited 1,000-game parent match scored
  `311/402/287` (51.20%, `+8.34 +/-15.29 Elo`). Fresh H18-PGO improved paired
  throughput by 2.97% over H15-PGO and scored 34.05% against Blunder versus
  H15-PGO's same-opening 31.85%. The paired +2.20-point interval (-1.15 to
  +5.50) crosses zero, so the external gain remains directional.
- SANJ feature analysis now accepts the current 39-column trace contract,
  including the fixed king-danger diagnostic column; its 42 tunable columns
  and production-score reconstruction remain unchanged.
- The occupied-bitboard NNUE adapter beat its frozen source-identical parent
  `341/393/290` in 1,024 audited equal-time games (52.49%,
  `+17.32 +/-14.64 Elo`, LOS 98.99%). Its separate clean 1,024-game Blunder
  8.0.0 screen scored `361/338/325` (51.76%, `+12.22 +/-17.11 Elo`). All
  2,048 games terminated normally without timing, legality, crash or protocol
  failure; the external score is not a paired improvement claim.

### Rejected

- Raising guarded null-move reduction from R3 to R4 only at depth eight and
  deeper reduced the depth-9 tree by 2.27%, but its clean audited 512-game
  parent screen scored `140/210/162` (47.85%, `-14.94 +/-19.79 Elo`). The
  playing change was removed without an external-engine screen.

- A follow-up that avoided a second `make_move` during Stockfish accumulator
  updates passed score-parity tests but tied the retained bitboard adapter
  `161/189/162` in 512 audited equal-time games (49.90%,
  `-0.68 +/-20.76 Elo`). Its larger playing diff was removed.

- H20 fused classical SANJ material, PSQT, phase, activity, pressure, and
  rook-file work into one color-local piece traversal. It preserved the exact
  depth-8 and depth-12 trees/checksums and improved 31-pair native median
  throughput by 3.47%, but its independently audited 1,000-game equal-time
  parent screen scored `300/386/314` (49.30%, `-4.86 +/-15.83 Elo`). The
  negative point estimate failed the registered gate, so the playing code was
  removed before PGO or a Blunder screen.
- H19 fused good-tactical, killer, and quiet active-index collection into each
  stage's existing classification pass. It preserved the exact 536,259-node
  depth-8 tree/checksum and improved 31-pair native median throughput by 2.88%,
  but its independently audited 1,000-game equal-time parent screen scored
  `302/382/316` (49.30%, `-4.86 +/-15.29 Elo`). The negative point estimate
  failed the registered gate, so the playing code was removed before PGO or a
  Blunder screen.
- H17's stage-local reuse of the SHEGERD material-phase bucket preserved the
  exact 536,259-node depth-8 tree and checksum, but across 31 interleaved
  native pairs its median rose from 256 ms to 259 ms and paired median
  throughput fell by 1.53%. It failed the registered engineering gate, so zero
  games ran and the playing code was removed.
- H16's search-local capture history passed focused correctness tests but
  expanded the native depth-8 tree from 536,259 to 605,581 nodes (+12.93%).
  Across 31 interleaved pairs its median rose from 257 ms to 299 ms and paired
  median throughput fell by 14.09%. It failed the registered engineering gate,
  so zero games ran and the playing code was removed.
- H13's exact 4,096-entry pawn cache preserved the depth-8 tree and checksum
  and reduced the 21-pair native median from 303 ms to 287 ms (5.57%), but its
  audited 1,000-game equal-time parent screen scored 49.40%
  (`-4.17 +/-15.98 Elo`). The negative point estimate failed the registered
  gate, so the playing cache was removed before any Blunder screen.
- H3m's one-ply check-evasion extension passed its 128-game hybrid-parent screen
  at 53.12% (`+21.74 +/-46.53 Elo`) but expanded the five-position depth-5 tree
  from 34,345 to 40,415 nodes (+17.67%) and scored only 23.24%
  (`-207.54 +/-41.60 Elo`) in its separate 256-game Blunder 7.6 screen. The
  playing change was removed; both matches completed without a candidate
  protocol, legality, crash, or timeout anomaly.
- H3n's dense alpha-outcome butterfly history expanded the depth-5 tree from
  34,345 to 35,172 nodes (+2.41%) and lost its 128-game hybrid-parent screen at
  42.97% (`-49.18 +/-46.65 Elo`, `32/50/46`). It was rejected before a Blunder
  match and its playing code was removed.
- H3p's qsearch-verified razoring grew the deterministic depth-8 tree from
  536,259 to 544,767 nodes (+1.59%), failing its pre-registered requirement for
  at least a 2% reduction. It was rejected before games and removed.
- H3q's two-ply deep LMR candidate initially won its 128-game parent screen,
  but failed the mate-in-three regression. The amended low-material guard kept
  a 58.20% parent score (53/32/43) yet fell to 23.63% (28/163/65,
  `-203.76 +/-42.83 Elo`) in a clean 256-game Blunder 7.6 screen. The playing
  change was rejected and removed.
- H3r preserved the deterministic search tree but scored 47.66% (31/37/60,
  `-16.30 +/-45.96 Elo`) in its clean 128-game fixed-node parent screen. The
  game-persistent butterfly history was rejected before Blunder testing and
  removed.
- H3s tested the single registered 15% SANJ/N7 residual mix against the selected
  10% setting. It scored 48.83% (36/39/53, `-8.14 +/-46.33 Elo`) in 128 clean
  fixed-node games and was rejected before Blunder testing.
- H3t's bounded search-local pawn correction expanded the deterministic
  depth-8 tree from 536,259 to 595,258 nodes (+11.00%), failing its registered
  2% ceiling. It was rejected before games and removed.
- H10's four-entry transposition clusters passed every correctness and
  concurrency test but reduced the depth-8 tree by only 0.006% and tied H9's
  295 ms median across 21 interleaved pairs. It failed both registered
  engineering floors, so no games ran and the playing code was removed.
- H11's outcome diagnostic kept both colors of each Blunder opening pair in
  one partition and removed ambiguous repeated positions. A scale fitted on
  98 training pairs worsened both cross-entropy and MSE on 30 unseen pairs,
  with both 95% group-bootstrap intervals crossing zero. No 42-weight fit or
  playing change was opened.

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
- The release binary reports its versioned UCI identity and passes `uciok` and
  `readyok`; the product label shown in this migrated changelog is NEYRANG.

### Rejected

- Guarded null-move pruning was reverted after its 1,000-game capped SPRT against the LMR parent remained inconclusive. Reverse futility pruning was not attempted in this release.

## [0.1.0] - 2026-08-29

- First stable UCI release with legal move generation, reversible state and hashing, classical evaluation, iterative alpha-beta/PVS search, quiescence, transposition table, time management, Perft fixtures, and deterministic benchmarking.
