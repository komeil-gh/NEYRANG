# REKHNE H4z: depth-aware same-key TT replacement

## Hypothesis

NEYRANG currently overwrites a current-generation TT entry whenever the same
position is stored again, even when the new non-exact result is much shallower.
That can discard expensive search evidence during transpositions. Current
Stockfish preserves deeper same-key entries unless the new result is exact or
within a small depth tolerance:
<https://github.com/official-stockfish/Stockfish/blob/master/src/tt.cpp>.

## Candidate

For both local and shared TT storage, keep existing empty-entry, generation,
different-key, exact-bound, and depth rules. For a current-generation same-key
entry, replace an exact result unconditionally; replace a non-exact result only
when it is no more than four plies shallower than the stored result. Keep the
one-entry indexing, layouts, signatures, score normalization, generation model,
and UCI Hash behavior unchanged.

## Gates

1. Direct local and shared-table regressions prove that a deep same-key entry
   survives a much shallower bound, an exact result replaces it, and a near-depth
   result still replaces it. All Rust tests, tactical tests, perft 5, formatting,
   and Clippy must pass.
2. The five-position hybrid depth-8 tree must not grow, every best-move/score
   signature must remain exact, and a changed tree must improve by at least 1%.
   An identical tree requires the registered 21-run 1% median speed gate.
3. A passing search change opens fresh independent 256-game fixed-node and
   equal-time parent screens against H4m; both require non-negative point
   estimates and zero anomalies before any Blunder screen.

Failure removes the playing code while retaining the measured outcome.
