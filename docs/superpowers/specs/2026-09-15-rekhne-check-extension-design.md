# REKHNE Check-Evasion Extension

## Hypothesis

NEYRANG currently enters the next nominal depth after every move, including a
move that leaves the opponent in check. Give an in-check main-search node one
extra ply so the engine resolves the forced evasion before spending the
remaining nominal depth. Quiescence remains unchanged because it already
searches every legal evasion.

## Boundary

The candidate changes one depth calculation in REKHNE. It does not change SANJ,
SHEGERD, move ordering, pruning margins, or artifacts. Mate, draw, stop, and
maximum-ply guards remain authoritative. Repeated checks may each earn an
extension, but repetition and `MAX_PLY` still bound the line.

## Gate

Build the candidate and the frozen `bcfaf7f` parent with identical `nnue,policy`
features and use the accepted policy plus `EvalMix=10` for both. Run 64 paired
openings at 20,000 nodes, one thread per engine, eight concurrent games, and
ponder off. Reject on a negative point estimate or any candidate anomaly. Only
a non-negative screen may proceed to a separate Blunder 7.6 match.

## Outcome

The 128-game parent screen scored 53.12% (`39/31/58`,
`+21.74 +/-46.53 Elo`) without a candidate anomaly, so the separate target
screen ran. Against Blunder 7.6, H3m scored 23.24% over 256 games
(`31/168/57`, `-207.54 +/-41.60 Elo`), below the previously measured hybrid
parent point estimate, while the five-position depth-5 tree grew from 34,345
to 40,415 nodes (+17.67%). H3m was rejected and its playing code removed.
