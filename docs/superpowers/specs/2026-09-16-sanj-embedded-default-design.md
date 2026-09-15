# SANJ H5k: embedded retained defaults

## Root cause

The accepted N7 residual and stage-aligned SHEGERD policy were used by the
strength campaign only when their files were passed explicitly. A normal
`cargo build --release` enabled neither feature and the resulting UCI binary
advertised neither artifact option. Users therefore received classical SANJ
and classical ordering instead of the tested playing stack. The residual
ablation had already measured classical SANJ at 43.36% against the selected
10% hybrid, about `-46 Elo` at that test condition.

## Change

The default feature set now enables `nnue` and `policy`, embeds the accepted N7
network and stage-aligned policy, and starts at `EvalMix=10`. UCI reports
`<embedded>` for both artifact defaults. `EvalFile=<empty>` and
`PolicyFile=<empty>` still provide explicit classical fallbacks, while
`<embedded>` restores the bundled assets. `--no-default-features` remains the
small classical-only developer build.

This changes out-of-box behavior only. It does not claim a new gain over the
already-configured accepted stack.

## Evidence

The focused default-build regression proves that both embedded assets load,
produce nontrivial evaluation and policy output, and can be disabled. The
default release build and full Rust suite pass. A direct 256-game fixed-node
screen against Blunder 7.6.0, with no `EvalFile` or `PolicyFile` options,
completed 48 wins, 50 draws, and 158 losses (28.52%, `-159.65 +/-43.31 Elo`).
All 128 color-reversed opening pairs completed normally at 20,000 nodes, one
thread, and 64 MiB hash. This is private regression evidence, not a formal
rating or proof that H5k is stronger than the explicitly configured parent.
