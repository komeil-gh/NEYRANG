# Architecture

## Boundaries

`NEYRANG` is the complete engine and UCI identity. `src/chess` owns
architecture-independent rules: compact types, bitboards, FEN, attacks, move
generation, make/unmake, Zobrist hashing, and Perft. `src/rekhne` owns the search
strategy: iterative deepening, alpha-beta/PVS/qsearch, TT, time limits, parallel
coordination, and stop checks. `src/sanj` judges positions without depending on
REKHNE. `src/shegerd` owns reusable strength techniques currently comprising
move ordering, history, and SEE. `src/uci` is the protocol boundary. `src/tools`
contains deterministic developer tooling, including the non-UCI OpenBench
`genfens` and SANJ trace command paths.

SANJ evidence code is isolated behind the non-default `sanj-tools` feature. Its
independent coefficient trace and TSV exporter do not enter the normal playing
binary. This boundary prevents corpus/training I/O and allocations from leaking
into UCI or REKHNE. Historical byte-identity claims remain attached to their
recorded pre-rename commits; the identity migration necessarily changes the
binary bytes.

The NNUE path is isolated behind the default-enabled `nnue` feature. The UCI
engine starts with the embedded retained N7 network at the accepted 10% residual
mix; an external `EvalFile` must still pass the fail-closed `NEYRANG\0` decoder,
and `<empty>` restores classical SANJ. Versions 1 and 2 retain one scalar output head; version 3
uses the same three-bank input transformer with four material-routed output
heads; experimental version 4 adds one post-SCReLU absolute-difference channel
to a single output head. Each searcher owns its accumulator stack; Lazy-SMP workers
share only an immutable `Arc` network and never share mutable accumulator state.
The optional `EvalMix` combines the incremental NNUE score with classical SANJ
using a bounded integer percentage; it changes neither accumulator ownership
nor the artifact format.
Ordinary moves, captures, en-passant, castling, promotions, re-searches, and
null moves all preserve the scalar full-refresh oracle, including the remaining
piece count used by version 3. The version-4 imbalance channel uses the existing
two accumulators and adds no mutable state. The engine
implementation is intentionally independent from `tools/nnue-reference`. SIMD
and default activation remain later evidence gates.

The default-enabled `policy` feature embeds the accepted stage-aligned
SHEGERD-P3 additive tables as a CPU move-ordering signal. P3 retains 75% of the
general P2 policy and adds a 25% Stockfish-18 teacher residual fitted from
current-engine decisions. External `PolicyFile`
artifacts are fail-closed and `<empty>` restores classical ordering. REKHNE supplies the previous-move
destination; SHEGERD reproduces the trainer's side normalization, material
phase, piece codes and SEE buckets. The policy may only rank moves inside an
existing MovePicker stage, so TT/PV priority, tactical SEE classification,
killer stages and legal-move completeness remain authoritative.

The engine has no runtime dependencies outside `std`. Strings, vectors, threads, and I/O remain at root/tool/protocol boundaries. A search node uses fixed move buffers and stack-based undo state.

## Position representation

Squares use `a1 = 0` through `h8 = 63`. The position keeps twelve piece bitboards, white/black/all occupancy, and a 64-entry mailbox. It also stores side to move, castling rights, en-passant target, halfmove/fullmove counters, cached king squares, and a deterministic 64-bit Zobrist key.

`make_move` updates every representation incrementally and returns only irreversible `UndoState`. `unmake_move` restores the parent without cloning. Debug/test integrity checks rebuild bitboards, occupancy, king squares, and the hash from the mailbox.

## Legal move generation

The production implementation generates pseudo-legal moves into `[Move; 256]`, computes the current checkers and absolute pins once, and accepts ordinary unpinned non-king moves directly when the side is not in check. King moves, pinned-piece moves, every en-passant capture, and all check evasions are validated against a simulated final occupancy and an enemy piece set with the captured piece removed. Merely enumerating legal moves no longer mutates the position.

The previous make/check/unmake filter remains compiled under tests as an immutable reference oracle. A deterministic 100,000-position corpus compares the exact legal list and order and explicitly records coverage of checks, double checks, pins, en passant, castling, and promotions. Castling still checks the origin, transit, and destination squares before emission and receives a final-occupancy safety check. En-passant discovered checks are handled by removing both the moving pawn's origin and the captured pawn before attack calculation.

Leaper attacks are compile-time tables. Bishops, rooks, and queens use portable
occupancy rays by default. A host-native x86-64 build with BMI2 instead indexes
once-initialized bishop and rook tables with `PEXT`; the scalar rays remain the
portable fallback, table builder, and independent test oracle. This backend is
an H14 deployment optimization, not a search or evaluation change.

## Concurrency and portability

`Threads=1` retains the established deterministic ownership model. The UCI input loop is separate from the search worker and signals stop through `AtomicBool` with relaxed operations; the flag carries no associated data and only requests cooperative cancellation. The position, repetition history, killers, history table, PV, and counters remain worker-owned. Its full-key TT is local and non-atomic, so the single-thread node path has no sharing cost.

The retained P3 path enables root-diversified Lazy SMP only when `Threads>1`. Every worker owns a position clone and private search state while sharing one total-Hash TT. A shared entry is one coherent relaxed `AtomicU64` containing a 16-bit key signature, packed move, normalized score, signed depth, generation, and bound. One-word publication prevents torn field combinations without a node-level lock. The 16-bit signature deliberately trades the local table's full-key certainty for twice as many entries per byte; a same-index signature collision can still produce a false hit and remains a documented design risk. The controller advances the generation once, starts helpers with distinct initial root preferences, runs the main iterative search, joins every helper, applies the registered root-move vote, emits one final aggregate info line and one bestmove, then returns the shared table to UCI ownership.

Timed parallel workers share one start instant and stop flag. Progress counters publish in 1,024-node chunks for live aggregate UCI info. Node-limited parallel searches reserve from one atomic total counter and stop at the exact aggregate budget. `ucinewgame`, Hash changes, Threads changes across the one/many boundary, position changes, and quit all stop and join active workers before clearing or replacing the table. The playing implementation uses no `unsafe` code and shares neither history nor PV state.

Distributed match concurrency remains outside the playing engine. The campaign
tool deterministically assigns each canonical opening to exactly one shard,
passes only sequential shard books to fastchess, and binds a worker to an
out-of-band campaign SHA plus exact engine, runner, and book hashes. A successful
worker emits a manifest for its PGN, log, match metadata, fastchess configuration,
and local audit. The coordinator verifies those hashes and reruns the PGN auditor
against the registered opening sequence before aggregating results. This is a
fixed-game evidence transport, not shared search state and not an SPRT server.

Opening generation is also outside search/UCI state. `src/tools/genfens.rs`
uses legal move generation and the frozen HCE to make a deterministic two-ply
reply-filtered walk. Each opening depends only on one complete unsigned 64-bit
seed, and batch index `i` maps to wrapping `seed + i`; OpenBench workers can
therefore split seed ranges without changing the resulting sequence. This first
version supports only `book None` and emits one exact
`info string genfens <fen>` line at a time. The independent Python wrapper
enforces the 15-second stall rule and publishes EPD plus provenance manifest
only after complete validation.

Scored self-play remains a one-way tool dependency. The standalone
`tools/nnue-data` crate imports NEYRANG's public chess/REKHNE APIs, but neither the
playing library nor UCI binary imports the recorder or codec. Its fixed-node
single-thread driver stores white-relative parent scores and accepts only
rules-complete games. A separate Python process validates the opening manifest,
assigns color-reversal-stable groups before generation, monitors progress,
replays the packed output through python-chess, verifies terminal WDL, and only
then publishes a no-clobber corpus plus manifest. No training-data I/O or
provenance branch enters the REKHNE hot path.

No x86 or ARM intrinsic is required by the portable build. Retained N7 uses a
runtime-checked AVX2 output kernel with the N2 scalar path as its bit-exact
fallback/oracle. The separately tested `target-cpu=native` deployment build
also enables H14's BMI2 slider tables. Its only `unsafe` operation is the
compile-time-gated `_pext_u64` intrinsic call; the portable ray backend and
independent exhaustive oracle remain authoritative. Native binaries are
machine-specific artifacts rather than portable releases.

H15 optionally adds profile-guided code layout to that same native artifact.
Its profile comes from the normal UCI path, but neither the profile nor the
optimized binary changes search or evaluation semantics. PGO artifacts remain
compiler-, workload-, and host-specific; the portable repository build is
still the release baseline.
