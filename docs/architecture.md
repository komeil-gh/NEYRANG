# Architecture

## Boundaries

`src/chess` owns architecture-independent rules: compact types, bitboards, FEN, attacks, move generation, make/unmake, Zobrist hashing, and Perft. `src/eval` evaluates a position without depending on search. `src/search` owns iterative deepening, qsearch, ordering, history, TT, time limits, and stop checks. `src/uci` is the protocol boundary. `src/tools` contains deterministic developer tooling.

The engine has no runtime dependencies outside `std`. Strings, vectors, threads, and I/O remain at root/tool/protocol boundaries. A search node uses fixed move buffers and stack-based undo state.

## Position representation

Squares use `a1 = 0` through `h8 = 63`. The position keeps twelve piece bitboards, white/black/all occupancy, and a 64-entry mailbox. It also stores side to move, castling rights, en-passant target, halfmove/fullmove counters, cached king squares, and a deterministic 64-bit Zobrist key.

`make_move` updates every representation incrementally and returns only irreversible `UndoState`. `unmake_move` restores the parent without cloning. Debug/test integrity checks rebuild bitboards, occupancy, king squares, and the hash from the mailbox.

## Legal move generation

The current implementation generates pseudo-legal moves into `[Move; 256]`, makes each move, rejects positions where the moving king is attacked, and unmakes it. This deliberately favors a small trustworthy legality path. Castling checks the origin, transit, and destination squares before emission. En-passant discovered checks are caught by the normal post-move legality test.

Leaper attacks are compile-time tables. Bishops, rooks, and queens use portable occupancy rays. A future magic or lookup backend must preserve the scalar implementation as a reference/fallback and demonstrate a benchmark gain on Apple Silicon.

## Concurrency and portability

Search is single-threaded. The UCI input loop is separate from the search worker and signals stop through `AtomicBool` with relaxed operations; the flag carries no associated data and only requests cooperative cancellation. Search ownership is transferred into the worker, so the position and TT need no node-level locks. When the worker finishes, its TT is returned to the UCI controller and reused by the next search; `ucinewgame` clears it and a Hash option change replaces it.

No x86 or ARM intrinsics are currently required. CPU-specific NNUE or attack code must stay behind isolated platform modules when introduced.
