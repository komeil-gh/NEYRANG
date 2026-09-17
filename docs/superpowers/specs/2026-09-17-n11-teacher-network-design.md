# N11: independent teacher-trained SANJ replacement

## Trigger evidence

The independent `bec082f` binary scored 27.54% against Blunder 7.6.0. The
earlier N10 corpus used 2,000-node labels and remained over-saturated even
after filtering; every resulting network lost to retained N7. Imported
Stockfish runtime evaluation was removed and is not an admissible shortcut.

## Corpus contract

Generate 16,000 seeded Stockfish-18 self-play games with one thread per worker,
20,000 fixed nodes per search, 64 MiB hash, hash clear before every game, up to
64 quiet records and eight saturated records per game. Partition whole games
80/10/10 by SHA-256 before global canonical-FEN deduplication. Train and
validation are usable for fitting and selection; holdout stays unopened until
one artifact, quantization, and EvalMix are frozen.

## Candidate lanes

Train exactly three 200-superbatch lanes from fresh deterministic weights:

1. `Chess768x3hm`, mean-squared-error loss.
2. `Chess768x3hm`, binary-cross-entropy loss.
3. `Chess768x3hm4ph`, mean-squared-error loss.

Use batch size 16,384, initial learning rate 0.001, cosine decay to 0.00000243,
and checkpoints every ten superbatches. Quantize checkpoints only with the
registered NEYRANG converter; screen QA 1536 first, then QA 511 and 255 only
for the best checkpoint in each lane.

## Selection and gates

On validation, evaluate every artifact with the independent reference decoder
and the exact production integer blend for EvalMix 0 through 100. An artifact
is eligible only if its best-mix BCE and MSE both improve on retained N7 at
EvalMix 10. Freeze the artifact with the lowest BCE, breaking ties by MSE then
lower EvalMix. Open holdout once and require the frozen artifact at the frozen
mix to improve both BCE and MSE over N7 at mix 10; do not reselect on holdout.

Then require artifact/runtime parity, the complete correctness suite, a clean
256-game paired fixed-node parent screen with score at least 50%, and a clean
512-game equal-time parent screen with score at least 50%. Only then may N11
replace N7 for the fresh equal-resource Blunder gate. Offline loss is selection
evidence, not Elo.

## Outcome

Rejected before holdout. The registered generator completed 16,000
Stockfish-18 self-play games and produced 540,672 training, 70,055 validation,
and 64,557 sealed holdout positions after whole-game partitioning and canonical
deduplication. All three 200-superbatch CUDA lanes completed. Sixty QA-1536
checkpoints and the preregistered QA-511/255 quantizations for each lane's best
checkpoint were evaluated with the independent decoder.

No candidate improved both validation BCE and MSE over retained N7 at its
production `EvalMix=10`. The strongest fresh candidate still had worse
validation loss, so no artifact or mix was eligible to freeze and the holdout
was never opened. Runtime parity and game gates were therefore not run; N7
remains retained.
