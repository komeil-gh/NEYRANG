# REKHNE H4k: selective singular extension

## Hypothesis

NEYRANG's short searches often identify one deep TT move without spending an
extra ply to verify the forced line behind it. A reduced exclusion search can
distinguish a genuinely singular move from an ordinary best move, extending
only the former instead of extending every check as rejected H3m did.

## Candidate

At a non-root depth of at least six, an exact or lower-bound TT move may open
one reduced null-window search of all alternatives when the entry is at least
`depth - 3` plies deep and its score is outside the mate band. The alternative
threshold is `tt_score - 2 * depth` and the exclusion depth is
`(depth - 1) / 2`. If every alternative stays below that threshold, only the TT
move receives one extra ply.

The excluded node cannot recurse into another singular probe, use a TT cutoff,
write a TT entry, or train killer/history state. It returns the probe alpha when
no alternative exists. Descendants retain normal search semantics. No double/triple or negative
extension, multi-cut, evaluator change, ordering change, artifact change, or
UCI default is bundled. This is a deliberately smaller adaptation of the
singular-move test documented in the current Stockfish search, not a copy of
its tuned margins or coupled heuristics.

## Gates

1. Full Rust tests, the 11-position tactical suite, start-position perft 5,
   Clippy, and a direct exclusion/restoration regression must pass.
2. The five-position hybrid depth-8 search may grow by at most 25%; no solved
   tactical position may regress and any changed move/score must be recorded.
3. A passing local gate opens 128 fresh, disjoint color-reversed pairs at
   20,000 nodes against H4c. Reject on an anomaly or negative point estimate.
4. Only a passing parent screen opens a separate 256-game Blunder 7.6.0 screen;
   it must exceed the retained 26.56% point estimate to claim progress.

## Outcome

Rejected. The original local screen passed all correctness checks but grew the
five-position hybrid depth-8 tree from 513,582 to 711,903 nodes (+38.61%), so it
was initially removed without games. After the retained N7 and policy artifacts
became the embedded defaults, the same bounded candidate was rebuilt and given
the missing game evidence. It scored 80 wins, 98 draws, and 78 losses over 256
fixed-node parent games (50.39%, `+2.71 +/-30.17 Elo`), then fell to 33 wins, 54
draws, and 169 losses against Blunder 7.6.0 (23.44%, `-205.64 +/-42.23 Elo`).
Both completed matches used 20,000 nodes, 128 color-reversed opening pairs, one
default engine thread, 64 MiB hash, and eight concurrent games. The Blunder
screen was rerun in strict mode without passing Blunder's unsupported `Threads`
option; the earlier compatibility-warning run is retained only as invalid
private evidence. Because the clean opponent score missed both the retained
26.56% gate and the current embedded-default 28.52% reference, the candidate
source and focused test were removed again.
