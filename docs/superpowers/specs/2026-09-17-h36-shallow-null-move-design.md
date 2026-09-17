# H36: guarded shallow null-move probe

## Trigger evidence

H35 preserves H30's search tree except in conservatively dead positions, and
the retained search still loses decisively to Blunder 7.6.0. The current null
move rule starts at depth four even though every existing safety guard already
excludes root, PV, check, mate-window, null-subtree, zugzwang-prone material,
low static evaluation, and positions without a legal move. A depth-three probe
is the smallest untested extension of the accepted rule and may remove shallow
fail-high branches before move generation.

## Candidate

Change only the minimum eligible null-move depth from four to three. Keep the
existing R2 reduction below depth six, R3 at depth six and above, and every
eligibility guard unchanged. Search, evaluation, ordering, artifacts, and UCI
defaults otherwise remain frozen on H35.

## Gates

1. Focused stats tests must prove depth-three probes occur in ordinary
   middlegames while every registered zugzwang, check, PV, root, mate-window,
   nested-null, stop, and position-restoration guard remains intact.
2. Formatting, clippy, the complete Rust suite, Perft 5, UCI smoke, and the
   deterministic benchmark must pass on the SSH worker.
3. A clean 512-game paired 50,000-node screen against frozen H35 must score at
   least 50% with no candidate anomaly. Record the tree change separately.
4. A passing fixed-node candidate receives a 512-game equal-time parent screen;
   its point estimate must be non-negative before any Blunder test.
5. Only both parent passes open a fresh same-opening 512-game Blunder 7.6.0
   screen. Retention requires exceeding H35's registered external point
   estimate without an engine, protocol, timing, or opening-pair anomaly.

## Outcome

Rejected and reverted. Formatting, clippy, the complete all-features Rust
suite, Perft 5, and UCI tests passed. The depth-8 tree grew from H35's 536,259
nodes to 583,244 nodes (+8.76%). H36 nevertheless passed both parent screens:

- fixed 50,000 nodes: 157 wins, 230 draws, 125 losses, **53.12%**,
  `+21.74 +/-20.88 Elo`, pentanomial `[11, 58, 92, 78, 17]`;
- equal time `0.5+0.005`: 148 wins, 218 draws, 146 losses, **50.20%**,
  `+1.36 +/-22.04 Elo`, pentanomial `[20, 57, 99, 61, 19]`.

Both parent audits accepted 512 normal colour-reversed games with zero
candidate anomaly. The same-opening external gate then failed: H36 scored 93
wins, 132 draws, and 287 losses against Blunder 7.6.0, **31.05%** and
`-138.55 +/-25.73 Elo`, below H35's registered 33.30%. The audit accepted only
two known opponent post-threefold PV warnings and found no NEYRANG failure.
Internal self-play therefore did not override the external regression; the
minimum null-move depth and focused candidate test were restored to H35.
