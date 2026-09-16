# SANJ H9: shared attack traversal

## Trigger evidence

Stockfish-18 replay of 12,229 external-game decisions shows a steep quality
gain with completed search depth, while the retained SANJ classical component
still traverses every non-pawn piece once for mobility and again for king
pressure. The two terms use the same attack set and occupancy.

## Candidate

Traverse each side's non-pawn pieces once per classical evaluation. Reuse each
exact attack bitboard for both the unchanged mobility score and pressure on the
opposing king. Keep pawn pressure, attacker counts, queen support, caps,
weights, phase interpolation, N7 mix, search, and every returned score frozen.
The independent SANJ trace remains unchanged and therefore serves as the
score oracle.

## Gates

1. Formatting, Clippy with warnings denied, the full Rust suite, and the
   100,000-position trace reconstruction must pass on SSH.
2. Across 21 interleaved host-native classical depth-8 pairs, node count and
   checksum must remain exact and median wall time must improve by at least 5%.
3. A 16-game 10,000-node UCI check must preserve paired outcomes and reported
   node/depth distributions while improving median NPS.
4. A passing engineering gate opens 1,000 equal-time games against H8. A
   positive point estimate and clean audit then open a same-opening 1,000-game
   Blunder 7.6.0 screen; external progress requires exceeding H8's 27.40%.

## Outcome

Accepted as H9.

- Formatting, Clippy with warnings denied, the full Rust suite, and both
  100,000-position oracles passed on the SSH host.
- The 21-pair host-native benchmark preserved the exact 536,259-node tree and
  `9d8d22b14e51010d` checksum. Median depth-8 time fell from 319 ms to 300 ms
  (6.33%).
- The 16-game fixed-10,000-node check preserved every result and the complete
  node/depth/seldepth multisets. Median reported NPS rose from 1,175,411 to
  1,220,612 (3.85%).
- H9 beat H8 `331/370/299` over 1,000 clean equal-time games: 51.60%,
  `+11.12 +/-16.20 Elo`.
- Against Blunder 7.6.0, the clean registered rerun scored `161/240/599`:
  28.10%, versus H8's same-opening 27.40%. The paired difference is +0.70
  percentage points (95% interval -2.50 to +3.90), or +6.07 logistic Elo
  (paired 95% interval -21.61 to +33.74). This external gain is not
  statistically proven.

An earlier Blunder run scored 30.65%, but its match metadata accidentally
retained the incompatible `reject-all` warning policy. It is preserved with a
`policy-mismatch` suffix and excluded from the acceptance result; no metadata
was edited after play.
