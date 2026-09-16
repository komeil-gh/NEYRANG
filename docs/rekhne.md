# REKHNE

REKHNE is the strategy NEYRANG searches with. Its code lives in `src/rekhne`;
ordering, history, and SEE are supplied by SHEGERD rather than owned by the
search driver.

## Current pipeline

NEYRANG uses iterative deepening over negamax alpha-beta. Non-first moves use a zero-window PVS search and are re-searched only when they improve alpha without failing high. From depth four onward, aspiration windows start at ±50 centipawns and widen geometrically on fail-low/high.

At depth zero, quiescence searches promotions and captures whose legal static exchange evaluation is non-negative. Positions in check do not use stand pat or SEE pruning and search every legal evasion. Mate scores encode root ply and are normalized when crossing the transposition-table boundary, so retrieval at a different ply preserves mate distance.

At non-root main-search nodes, exact mate-distance bounds close windows that
cannot improve a shorter mate already proved by an ancestor. The bounds follow
NEYRANG's existing root-ply mate encoding and do not use a tuned margin.

Main-search move ordering is delivered progressively by a fixed-capacity staged MovePicker:

1. TT/PV move
2. promotions and non-losing captures, scored once with SEE plus MVV-LVA
3. killer moves
4. quiet history
5. losing captures

The default build adds the accepted bounded six-table SHEGERD score inside
these stages. It reuses the SEE value already computed for tactical moves and
the previous move's destination for contextual quiet ordering. It never moves a
candidate across the TT/PV, good-tactical, killer, quiet, or losing-tactical
boundaries. `PolicyFile=<empty>` restores the registered classical ordering.

Each stage is initialized only when search reaches it; a cutoff can therefore avoid scoring every later quiet or tactical move. The picker suppresses preferred/killer duplicates and returns every legal candidate at most once. Tactical classification still uses exact SEE. The oracle-equivalent `see_ge` primitive is retained and tested, but the experimental lazy main-search caller was reverted, so current production ordering does not silently substitute threshold classification for exact exchange scores.

SEE builds one compact exchange board, computes attackers to the fixed target once, then carries piece/color occupancy and the attacker set through each recapture. Vacated sources reveal only the relevant diagonal or orthogonal slider x-rays, and the accepted legal least-valuable-attacker state is prepared once. Full king safety remains authoritative for every candidate; promotions, en passant, absolute pins, illegal king recaptures, piece-kind/Lsb LVA order, exact scores, and threshold answers retain their previous semantics. The complete pre-G1 algorithm remains a test-only oracle. Qsearch retains exact SEE classification and its established losing-capture pruning.

At non-root, non-PV nodes, sufficiently late quiet moves may be reduced by one ply. The reduction starts with the fifth searched move at depth three, excludes TT moves, killers, strong-history moves, captures, promotions, checks, and nodes already in check, and always performs a normal-depth zero-window re-search when the reduced result raises alpha. The ordinary PVS full-window re-search remains authoritative when needed.

At depths one through three, H3f can stop the ordinary-quiet stage after `3 + depth * depth` searched moves once at least one non-mating line is known. Root and PV nodes, nodes in check, preferred moves, killers, promotions, captures, and every later bad-capture candidate remain searchable.

H3g adds a separate parent-side futility gate at non-root, non-PV depths one through four. When static SANJ plus `100 * depth` cannot reach alpha, it still searches the first candidate and preserves checks, preferred moves, killers, captures, promotions, mate-score windows, low-material endings, and null subtrees while skipping later ordinary quiets.

H3h reuses the exact SEE value already computed by the staged MovePicker. At non-root, non-PV depths through six, a later non-promotion capture below `-100 * depth` is skipped without another SEE call. The first move, preferred move, checks, PV nodes, nodes in check, null subtrees, promotions, and mate-loss defense remain searchable.

The retained development branch also uses conservative depth-scaled null-move pruning at eligible null-window nodes: R2 at depths four and five, then R3 from depth six. It requires no check or earlier null, a non-mate beta, static evaluation at least beta, and meaningful friendly non-pawn material. Pawn-only and lone-minor endings are excluded. Legal terminal detection precedes the probe. Synthetic null descendants cannot use real-game repetition/fifty-move adjudication, the TT, or persistent killer/history training, and null state is restored exactly. There is no verification search in this variant.

Before the null-move probe, a separately tested reverse-futility guard can return the static SANJ score at non-root, non-PV depths one through three when it exceeds beta by at least `150 * depth`. The guard is disabled in check, null subtrees, mate-score windows, low-material endings, and nodes whose TT move is quiet. Legal terminal detection remains authoritative.

Static SANJ is evaluated lazily at most once per main-search node and reused by
reverse futility, null move, and forward futility when more than one guard needs
the same score. This cache is stack-local and does not change search decisions.

## Parallel search

P3 implements root-diversified Lazy SMP behind `Threads>1`; `Threads=1` continues through the old local table and search entry point. The main worker starts with the established root order. Helper `i` starts with legal root move `i mod root_move_count`, then private histories and the shared TT allow searches to diverge naturally. Only the main worker emits iterative information. After all workers finish, root moves receive the registered score vote `score - minimum_score + 14`; vote ties use completed depth, PV length, and main-worker priority. The chosen representative contributes score/PV while nodes, qnodes, seldepth, elapsed time, hashfull, and search statistics are aggregated.

The shared TT uses one atomic word per entry, relaxed operations, a 16-bit signature, and one generation advanced by the controller. Mate scores are normalized at the same boundary as the local TT. Timed workers share a start instant before helper creation, so spawn overhead consumes the budget. Node-limited searches use one exact global budget. Frozen P3 passed its 1/2/4-thread scaling and deadline gates, then scored 53.075% in the complete audited 2,000-game Threads-2-versus-Threads-1 screen. The path is retained on the development branch; more cores are extra compute, not a free single-thread efficiency claim or a release-wide Elo promise.

## Draws and limits

The search checks the 50-move counter and counts matching hashes within the reversible history window for threefold repetition. Maximum ply is 128. At the boundary it returns static evaluation rather than indexing past fixed search storage.

The UCI thread can set an atomic stop flag while search is active. Local node limits are checked every node; the parallel path reserves against one exact aggregate counter. Hard time is sampled at the first node, every node for budgets of at most 5 ms, and every 1,024 nodes otherwise. Soft time is evaluated after completed iterations. A legal root fallback is retained even when the usable budget is zero, and an interrupted iteration never replaces the last stable result. The default configurable `Move Overhead` is 30 ms after the [0.3 timing audit](development/0.3.0-timing-audit.md) measured the external macOS/runner latency tail.

## Measured development changes

Single release runs on the same Apple Silicon host, five positions, depth 5:

| Revision step | Nodes | Time | Outcome |
| --- | ---: | ---: | --- |
| Alpha-beta + TT baseline | 1,060,548 | 832 ms | baseline |
| Killer + quiet history | 664,949 | 603 ms | retained |
| SEE inside comparator | 719,072 | 971 ms | reverted |
| PVS | 447,006 | 286 ms | retained |
| Aspiration ±25 | 477,104 | 317 ms | retuned |
| Aspiration ±50 | 448,136 | 282 ms median | retained |
| Precomputed SEE ordering | 528,816 | 464 ms median | retained after positive match screens |
| Qsearch SEE pruning | 359,538 | 203 ms median | retained |
| Conservative LMR | 196,627 | 166 ms median | retained and release-proven cumulatively |
| Staged MovePicker | 193,669 | 158 ms phase median | retained after 7,046 valid games |
| Conservative fixed-R2 NMP | 180,591 | 126 ms phase median | retained after 7,946 valid games |

The final row is the median of five runs; the preceding rows are single-run development snapshots. These are engineering measurements, not Elo evidence. Search-tree changes make raw NPS comparisons insufficient; paired games and SPRT decide strength patches.

The later F1 legality-filter and G1 exact-SEE optimizations preserve the final 180,591-node depth-5 tree and the 4,483,694-node depth-8 tree exactly. G1 reduced the binding 15-run depth-8 median from 2,341 to 2,237 ms (-4.44%) against frozen F1, then completed a clean strict 2,000-game regression screen at 50.98%. This is score-preserving throughput evidence, not a general Elo estimate.

## Experiment outcome and next search work

The first 0.2 guarded null-move candidate reduced the benchmark to 182,768 nodes but did not accept H1 in a capped 1,000-game SPRT against the LMR parent, so that historical implementation was reverted. Reverse futility pruning was not attempted in 0.2.0.

The [0.3 retrospective campaign](development/0.3.0-game-campaign.md) completed 4,000 fixed games across the preserved 0.1/SEE/qsearch/LMR/0.2 binaries. SEE ordering measured `+29.25 +/-17.09 Elo`, qsearch SEE pruning measured `+66.46 +/-18.72 Elo`, isolated LMR remained inconclusive at `+6.25 +/-12.52 Elo`, and cumulative v0.2.0 measured `+90.97 +/-18.38 Elo` against v0.1.0. Five 2-11 ms time forfeits in historical parents triggered a separate timing audit. Exact v0.2.0 reproduced strict zero-margin losses; isolated hardening closed with a clean 1,000-game timing stress and defined `BASE_0_3` at `303711a`.

The 0.3 ordering work then kept the staged MovePicker after 7,046 valid comparison games. A real oracle-equivalent `see_ge` primitive was verified, but the only registered lazy main-search caller was rejected after 12,000 valid games: its final 10,000-game normalized SPRT capped at LLR `+0.69` inside the decision bounds. Commit `2e9637e` restored the accepted MovePicker/B0 search behavior while retaining the unused threshold primitive.

The single registered Capture History candidate then completed 2,000 valid screening games. Its node-limited and equal-time scores were 50.40% and 50.05%, but it expanded the deterministic benchmark tree from 193,669 to 201,535 nodes (+4.06%) and increased median wall time from 130 to 135 ms (+3.85%). That crossed the pre-registered rejection boundary without compensating tree reduction, so commit `f5450ec` removed the feature.

The following 1-ply Continuation History candidate completed 2,000 diagnostic games. Its node screen was exactly neutral and its recovered equal-time score was 50.80%, but a strict candidate timeout was binding under the registered protocol. Commit `3a9e2d9` restored the same 569,504-byte parent, SHA-256 `4ade6870655269142b1be7f8b597f27c9e16beb267732973b157bdee9ad9f15e`.

The second conservative NMP experiment was then run against that restored ordering baseline. E1 accepted H1 after 4,946 fresh normalized-SPRT games and scored 54.95% in a separate clean 1,000-game confirmation at `8+0.08`; it was retained after 7,946 valid games. The cumulative candidate reduced the immutable v0.2.0 benchmark tree by 8.16% and median wall time by 11.97%. Release validation nevertheless stopped after both permitted primary-screen attempts triggered reproducible repetition-invalid PV output from immutable v0.2.0. No v0.3.0 version or tag was created.

P3 then retained one root-diversified Lazy-SMP design without changing the Threads-1 tree. Threads 2/4 reached `2.010776x / 3.956388x` the Threads-1 aggregate NPS in the frozen external scaling gate, with p95 hard-deadline overshoot below 0.7 ms. Its binding same-binary 2,000-game screen completed `711/701/588` for Threads 2, 53.075%, reported `+21.39 +/-10.90 Elo`, with all 1,000 pairs and 196,758 plies independently replayed and no timing, legality, crash, warning, or protocol anomaly. A normalized SMP SPRT remains a separately registered future decision.

T1 subsequently moved the monotonic start timestamp for every timed search to
the instant the UCI `go` command enters the engine, so operating-system worker
dispatch is covered by the hard deadline. It preserved both deterministic
trees, passed 1,500 direct deadline samples across Threads 1/2/4, and completed
1,000 fresh same-binary games with zero timing or protocol anomaly. T1 is
retained as infrastructure only; it provides no strength or Elo claim and does
not retroactively prove the cause of the interrupted S4 games.

The next isolated NMP refinement changed only deep eligible probes from R2 to
R3, starting at depth six. It reduced the deterministic depth-8 tree from
3,984,069 to 3,691,765 nodes (-7.34%). Audited 1,000-game paired UHO screens
were neutral-positive at both 20,000 fixed nodes (`+1.04 +/-3.91 Elo`) and
`0.5+0.005` equal time (`+3.13 +/-16.71 Elo`), so the cheaper search was
retained without claiming a statistically proven Elo gain. Starting R3 one ply
earlier was rejected after its separate 1,000-game equal-time screen scored
`-6.95 +/-14.45 Elo`.

The next isolated H3e candidate added the shallow reverse-futility guard above.
It reduced the deterministic depth-8 tree from 3,691,765 to 1,712,758 nodes
(-53.60%) and the default depth-5 OpenBench tree from 165,444 to 105,187
nodes. Independent audits accepted both 1,000-game paired UHO gates: the
20,000-node screen scored 54.70% (`+32.76 +/-15.95 Elo`) and the
`0.5+0.005` equal-time screen scored 57.05% (`+49.32 +/-16.85 Elo`). All
2,000 games terminated normally with zero timing, legality, crash, warning, or
protocol anomaly, so H3e was retained.

H3f then added the guarded shallow late-move rule above. It reduced the
depth-8 tree again, from 1,712,758 to 687,111 nodes (-59.89%), and the default
depth-5 OpenBench tree from 105,187 to 45,540 nodes. Independent audits accepted
both 1,000-game paired UHO gates: the 20,000-node screen scored 63.70%
(`+97.69 +/-17.84 Elo`) and the `0.5+0.005` equal-time screen scored 55.90%
(`+41.19 +/-16.93 Elo`). All 2,000 games terminated normally with no timing,
legality, crash, warning, or protocol anomaly, so H3f was retained.

H3g then reduced the retained depth-8 tree from 687,111 to 646,941 nodes
(-5.85%) and the default depth-5 OpenBench tree from 45,540 to 44,199 nodes.
Its independently audited 1,000-game gates scored 53.15%
(`+21.92 +/-17.00 Elo`) at 20,000 nodes and 50.70%
(`+4.86 +/-16.01 Elo`) at `0.5+0.005`. All 2,000 games terminated normally
without a timing, legality, crash, warning, or protocol anomaly. H3g therefore
met its pre-registered non-negative point-estimate floor, but the equal-time
result is not a statistically proven Elo gain.

H3h reused staged SEE to reduce the retained depth-8 tree from 646,941 to
567,113 nodes (-12.34%) and the default depth-5 OpenBench tree from 44,199 to
35,176 nodes. Its independently audited 1,000-game gates scored 51.90%
(`+13.21 +/-16.17 Elo`) at 20,000 nodes and 50.90%
(`+6.25 +/-17.43 Elo`) at `0.5+0.005`. All 2,000 games terminated normally
without a timing, legality, crash, warning, or protocol anomaly. H3h met the
pre-registered non-negative point-estimate floor; neither Elo gain is
statistically proven.

H8 then froze the complete H7 source and changed only deployment code
generation to `-C target-cpu=native` on the registered x86-64 worker. Exact
classical and pure-N7 trees and checksums were preserved while their 21-pair
median wall times improved by 8.41% and 25.10%. In a clean audited 1,000-game
equal-time match H8 beat the portable H7 binary at 59.50%
(`+66.82 +/-17.17 Elo`). Against Blunder 7.6.0 it raised the same-opening score
from H7's 23.90% to 27.40%; the paired difference was +3.50 percentage points
(95% interval +0.47 to +6.53), or +31.92 logistic Elo (paired 95% interval
+4.11 to +59.73). H8 is retained only as a host-specific deployment artifact;
the ordinary build remains portable.

H9 then removed SANJ's duplicate non-pawn attack traversal while preserving
every evaluation score. The exact depth-8 tree and checksum remained unchanged,
the 21-pair host-native median fell from 319 ms to 300 ms, and H9 beat H8 at
51.60% in 1,000 clean equal-time games. Its same-opening Blunder score rose
from 27.40% to 28.10%, but the paired 95% interval (-2.50 to +3.90 percentage
points) crosses zero; this is retained throughput work, not a proven external
Elo gain.

H12 then reused the move generator's occupancy-aware attacker query to bypass
recursive SEE when the destination cannot be recaptured. It preserved the exact
H9 depth-8 tree and checksum while reducing the 21-pair host-native median from
330 ms to 313 ms (5.43%). Its audited 1,000-game parent match scored 50.45%,
and its same-opening Blunder score rose from 28.10% to 29.75%. The paired
difference was +1.65 percentage points with a 95% interval of -1.70 to +5.10,
so the external gain is directional rather than statistically proven.

H13's exact pawn cache was then rejected after a negative 49.40% parent point
estimate despite a 5.57% speedup, and its playing code was removed. H14 instead
targeted the shared slider-attack hot path. Native BMI2 tables preserved the
same tree and checksum while reducing the 21-pair median from 289 to 257 ms
(12.45%). H14 beat H12 at 52.10% (`+14.60 +/-16.02 Elo`) in 1,000 clean games
and scored 30.00% against Blunder versus H12's same-opening 29.75%; that
external +0.25-point difference remains directional only.

H15 then kept H14 source-identical and used 256 normal fixed-node self-play
games only to guide native compiler layout. Perft, the 536,259-node depth-8
tree, and checksum remained exact. Across 31 interleaved pairs, median time
fell from 259 to 251 ms and paired median throughput improved by 3.23%. H15
scored 52.15% (`+14.95 +/-16.63 Elo`) against H14 in 1,000 clean equal-time
games. Its same-opening Blunder score rose from 30.00% to 31.85%; the paired
gain was +1.85 percentage points (95% interval -1.20 to +4.95), so the
external gain remains directional rather than statistically proven. H15 is a
host-specific deployment artifact, not a portable release build.

H16 tested search-local capture history inside the existing SEE-defined
tactical stages. Focused tests passed, but the depth-8 tree expanded by 12.93%
and paired median throughput fell by 14.09%. It failed the registered
engineering gate, so zero games ran and the playing code was removed.

NEYRANG does not learn merely by playing games; improvements still require an explicit, tested patch. Any future baseline-protocol repair, null-move refinement, LMP, ProbCut, or singular-extension work must be pre-registered and isolated rather than bundled into the accepted candidate. The complete outcome is in the [0.3 final report](development/0.3.0-final-report.md).
