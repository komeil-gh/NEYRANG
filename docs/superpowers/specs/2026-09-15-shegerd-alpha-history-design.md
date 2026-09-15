# SHEGERD Alpha-Outcome History

## Hypothesis

The current butterfly history learns only when a quiet move causes a beta
cutoff. Record every searched quiet move: reward one that raises alpha and
penalize one that does not. This supplies denser evidence to later MovePickers
without adding a table or changing stage boundaries.

## Boundary and gate

Captures, promotions, and synthetic null subtrees do not train the table.
Existing bounded history updates remain unchanged. Build this candidate and
the frozen `bcfaf7f` parent with identical `nnue,policy` features, accepted
policy artifact, and `EvalMix=10`. Run 64 paired openings at 20,000 nodes with
one thread per engine and eight concurrent games. Reject on a negative point
estimate, a deterministic-tree regression above 10%, or any candidate anomaly.
Only a passing candidate proceeds to a separate Blunder 7.6 screen.

## Outcome

The candidate expanded the five-position depth-5 tree from 34,345 to 35,172
nodes (+2.41%) and scored 42.97% over 128 parent games (`32/50/46`,
`-49.18 +/-46.65 Elo`). H3n failed the non-negative point-estimate gate, so no
Blunder match ran and its playing code was removed.
