# H35: conservative dead-position draws

## Trigger evidence

REKHNE recognized stalemate, the fifty-move rule, and threefold repetition, but
continued to evaluate positions where neither side can ever mate. Fastchess can
adjudicate those games externally; the engine still searched and valued the
same positions incorrectly inside its own tree.

The H34 loss report is not used to justify this change. Its retained CSV has
1,788 rows where the played move equals the reported best move but the reported
loss is nonzero, including 37 nominal severe errors. That diagnostic is
invalid for exact error counts until regenerated.

## Candidate

Return a draw for the conservative exact subset: bare kings, a single knight
or bishop against a bare king, and bishop-only positions where every bishop is
on the same square colour. Do not classify two knights, mixed minors,
opposite-colour bishops, or any position containing a pawn, rook, or queen.

Search, evaluation, ordering, pruning, artifacts, and UCI defaults are frozen.

## Gates

1. A focused regression must accept every registered dead case, reject every
   conservative boundary case, and prove search returns zero.
2. Formatting, the complete Rust suite, Perft, UCI smoke, and the deterministic
   benchmark must pass on the SSH worker.
3. A clean 512-game paired 50,000-node screen against frozen H30 must score at
   least 49% with no engine, protocol, timing, or opening-pair anomaly.
4. Only a non-negative parent point estimate may open a fresh equal-resource
   Blunder screen. Correctness retention alone is not an Elo claim.

## Outcome

Accepted as a correctness repair. The focused regression covers the retained
dead subset and the conservative boundary cases. The complete Rust suite,
all-features suite, Perft 5 (`4,865,609`), UCI smoke, and the deterministic
depth-8 benchmark (`536,259` nodes, checksum `9d8d22b14e51010d`) passed on the
SSH worker. A fresh target directory with test-profile LTO disabled avoided a
Rust 1.98.1 ThinLTO compiler ICE seen in the first all-features invocation.

The audited parent screen completed 512 normal games with zero warning,
forfeit, timing, protocol, or opening-pair anomaly. H35 scored 135 wins, 244
draws, and 133 losses: **50.20%**, `+1.36 +/-3.25 Elo`, pentanomial
`[0, 2, 250, 4, 0]`. This passes the 49% retention floor and the non-negative
point-estimate gate. The unusually narrow nominal interval comes from 250 of
256 colour-reversed pairs tying exactly; it is reported as runner output, not
as evidence of a material Elo gain.

The comparable external gate used the same `0.5+0.005` control, first 256 UHO
pairs, one NEYRANG thread, and exact Blunder 7.6.0 binary as the registered H30
run. H35 scored 91 wins, 159 draws, and 262 losses: **33.30%**,
`-120.67 +/-25.68 Elo` relative to Blunder. The audit accepted all 512 normal
games with no warning or candidate anomaly. An earlier 50,000-node external
attempt was interrupted by Blunder's known post-threefold PV warning and is
not comparable to this time-controlled baseline.
