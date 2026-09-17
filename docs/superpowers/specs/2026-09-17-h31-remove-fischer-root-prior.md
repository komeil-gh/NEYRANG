# H31: remove the unproven Fischer root prior

## Trigger evidence

The embedded Fischer prior changes the initial root move after ply 12, but the
current history contains no registered game gate or retained outcome for that
runtime behavior. Its training and unit tests establish provenance and
determinism, not playing strength. H30 still scores only 33.79% against Blunder,
so an unmeasured style preference cannot remain in the release by assumption.

## Candidate

Remove only the Fischer prior from the runtime root fallback. Keep the H30
search, N7 `EvalMix=10`, P3 policy, build flags, openings, limits, and all other
behavior frozen. The ordinary first legal move is used only until iterative
deepening completes a depth, matching the engine's pre-prior fallback contract.

## Gate

1. A 512-game paired 50,000-node match against H30 must have no candidate
   anomaly and a non-negative point estimate.
2. If it passes, a fresh 512-game paired `0.5+0.005` match must also have no
   candidate anomaly and a non-negative point estimate.
3. Only if both gates pass is the runtime prior removed. If either point
   estimate is negative, retain it and record the rejection. No external
   opponent match is justified for a rejected candidate.

## Outcome

Rejected on the first gate. The complete 512-game fixed-node match scored
133 wins, 134 losses, and 245 draws for the removal candidate: 49.90%,
-0.68 +/- 2.97 Elo, and pentanomial `[1, 0, 254, 1, 0]`. The independent audit
found all 512 games normal, all 256 opening pairs present, balanced colors,
matching options, and no warning, crash, timeout, protocol, or legality
anomaly. The negative point estimate failed the registered gate, so no
equal-time or external-opponent match ran and the runtime prior remains.
