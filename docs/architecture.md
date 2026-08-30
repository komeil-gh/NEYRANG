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

Search is single-threaded. The UCI input loop is separate from the search worker and signals stop through `AtomicBool` with relaxed operations; the flag carries no associated data and only requests cooperative cancellation. Search ownership is transferred into the worker, so the position and TT need no node-level locks. When the worker finishes, its TT is returned to the UCI controller and reused by the next search; `ucinewgame` clears it and a Hash option change replaces it.

No x86 or ARM intrinsics are currently required. CPU-specific NNUE or attack code must stay behind isolated platform modules when introduced.
