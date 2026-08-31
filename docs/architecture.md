# Architecture

## Boundaries

`src/chess` owns architecture-independent rules: compact types, bitboards, FEN, attacks, move generation, make/unmake, Zobrist hashing, and Perft. `src/eval` evaluates a position without depending on search. `src/search` owns iterative deepening, qsearch, ordering, history, TT, time limits, and stop checks. `src/uci` is the protocol boundary. `src/tools` contains deterministic developer tooling.

Evaluation evidence code is isolated behind the non-default `eval-tools` feature. Its independent coefficient trace and TSV exporter do not enter the normal playing binary; the default H0 build is byte-identical to the frozen G1 executable. This boundary prevents corpus/training I/O and allocations from leaking into UCI or search.

The engine has no runtime dependencies outside `std`. Strings, vectors, threads, and I/O remain at root/tool/protocol boundaries. A search node uses fixed move buffers and stack-based undo state.

## Position representation

Squares use `a1 = 0` through `h8 = 63`. The position keeps twelve piece bitboards, white/black/all occupancy, and a 64-entry mailbox. It also stores side to move, castling rights, en-passant target, halfmove/fullmove counters, cached king squares, and a deterministic 64-bit Zobrist key.

`make_move` updates every representation incrementally and returns only irreversible `UndoState`. `unmake_move` restores the parent without cloning. Debug/test integrity checks rebuild bitboards, occupancy, king squares, and the hash from the mailbox.

## Legal move generation

The production implementation generates pseudo-legal moves into `[Move; 256]`, computes the current checkers and absolute pins once, and accepts ordinary unpinned non-king moves directly when the side is not in check. King moves, pinned-piece moves, every en-passant capture, and all check evasions are validated against a simulated final occupancy and an enemy piece set with the captured piece removed. Merely enumerating legal moves no longer mutates the position.

The previous make/check/unmake filter remains compiled under tests as an immutable reference oracle. A deterministic 100,000-position corpus compares the exact legal list and order and explicitly records coverage of checks, double checks, pins, en passant, castling, and promotions. Castling still checks the origin, transit, and destination squares before emission and receives a final-occupancy safety check. En-passant discovered checks are handled by removing both the moving pawn's origin and the captured pawn before attack calculation.

Leaper attacks are compile-time tables. Bishops, rooks, and queens use portable occupancy rays. A future magic or lookup backend must preserve the scalar implementation as a reference/fallback and demonstrate a benchmark gain on Apple Silicon.

## Concurrency and portability

`Threads=1` retains the established deterministic ownership model. The UCI input loop is separate from the search worker and signals stop through `AtomicBool` with relaxed operations; the flag carries no associated data and only requests cooperative cancellation. The position, repetition history, killers, history table, PV, and counters remain worker-owned. Its full-key TT is local and non-atomic, so the single-thread node path has no sharing cost.

The pre-registered P3 development candidate enables root-diversified Lazy SMP only when `Threads>1`. Every worker owns a position clone and private search state while sharing one total-Hash TT. A shared entry is one coherent relaxed `AtomicU64` containing a 16-bit key signature, packed move, normalized score, signed depth, generation, and bound. One-word publication prevents torn field combinations without a node-level lock. The 16-bit signature deliberately trades the local table's full-key certainty for twice as many entries per byte; a same-index signature collision can still produce a false hit and remains part of the P3 playing-strength risk. The controller advances the generation once, starts helpers with distinct initial root preferences, runs the main iterative search, joins every helper, applies the registered root-move vote, emits one final aggregate info line and one bestmove, then returns the shared table to UCI ownership.

Timed parallel workers share one start instant and stop flag. Progress counters publish in 1,024-node chunks for live aggregate UCI info. Node-limited parallel searches reserve from one atomic total counter and stop at the exact aggregate budget. `ucinewgame`, Hash changes, Threads changes across the one/many boundary, position changes, and quit all stop and join active workers before clearing or replacing the table. The playing implementation uses no `unsafe` code and shares neither history nor PV state.

Distributed match concurrency remains outside the playing engine. The campaign
tool deterministically assigns each canonical opening to exactly one shard,
passes only sequential shard books to fastchess, and binds a worker to an
out-of-band campaign SHA plus exact engine, runner, and book hashes. A successful
worker emits a manifest for its PGN, log, match metadata, fastchess configuration,
and local audit. The coordinator verifies those hashes and reruns the PGN auditor
against the registered opening sequence before aggregating results. This is a
fixed-game evidence transport, not shared search state and not an SPRT server.

No x86 or ARM intrinsics are currently required. CPU-specific NNUE or attack code must stay behind isolated platform modules when introduced.
