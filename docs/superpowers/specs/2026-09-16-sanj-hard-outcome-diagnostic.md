# SANJ H11: hard-position outcome diagnostic

## Trigger evidence

Stockfish-18 analysis of 12,229 decisions from the retained A2CE55A versus
Blunder match found 954 errors of at least 100 centipawns after excluding mate
sentinels. H11 asks whether the existing classical SANJ feature basis is
miscalibrated on positions reached in those real losses. It is a diagnostic,
not authorization to tune on the external opponent.

## Data boundary

Map every labelled FEN back to its exact game and WDL result in the retained
256-game PGN. Keep both color-reversed games from a `Round` opening pair in the
same deterministic train or validation partition. Remove mate sentinels,
positions shared ambiguously by both games, and duplicate FENs. Do not inspect
the sealed final holdout.

The resulting private diagnostic corpus contains 8,551 positions from 98
training pairs and 2,613 positions from 30 validation pairs. All FENs mapped
back to the PGN; 822 mate sentinels and 243 result-ambiguous positions were
excluded.

## Tooling correction

The current Rust trace exports 39 columns after bounded king danger was added,
while the Python analyzer still required the former 38-column header. The
analyzer now accepts `king_danger_delta` as a fixed diagnostic column while
preserving its existing 42-column tunable design matrix. The header error uses
the schema length instead of another hard-coded count. All 16 focused analyzer
and fitter tests pass on the SSH worker.

## Outcome

Rejected before fitting evaluation weights or running games.

On the 98 training pairs, a single fitted score scale improved cross-entropy
from `0.5723978931` to `0.5622535512` and MSE from `0.1294445715` to
`0.1277612165`. On the 30 unseen validation pairs, it instead worsened
cross-entropy from `0.5419825280` to `0.5422716608` and MSE from
`0.1271189556` to `0.1281799676`. The opening-pair bootstrap 95% intervals for
the validation deltas were `[-0.0193138936, 0.0156938346]` and
`[-0.0023259469, 0.0043662263]`; both cross zero. A 42-weight fit would add
variance to an already non-generalizing signal, so no playing source changed.
