#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd -- "$script_dir/.." && pwd)"
scratch="$(mktemp -d)"
trap 'rm -rf "$scratch"' EXIT

pgn_out="$scratch/smoke.pgn"
meta_out="$scratch/smoke.meta.txt"
engine_log_out="$scratch/engine.log"

output="$(
    DRY_RUN=1 \
    FASTCHESS_BIN=true \
    ENGINE_A=/usr/bin/true \
    ENGINE_B=/usr/bin/true \
    ENGINE_A_GIT_SHA=candidate-sha \
    ENGINE_B_GIT_SHA=baseline-sha \
    ENGINE_A_EVAL_MIX=100 \
    ENGINE_B_EVAL_MIX=10 \
    OPENINGS_FILE="$repo_root/scripts/openings.epd" \
    OPENING_SEED=20260829 \
    TIME_MARGIN_MS=0 \
    MOVE_OVERHEAD_MS=30 \
    SHOW_LATENCY=1 \
    STRICT=1 \
    AUTOSAVE_INTERVAL=2 \
    CAMPAIGN_ID=campaign-identity \
    SHARD_ID=shard-0007 \
    CAMPAIGN_MANIFEST_SHA256=campaign-manifest-hash \
    SHARD_MANIFEST_SHA256=shard-manifest-hash \
    PAIR_OFFSET=14 \
    GAMES=10 \
    NODES=5000 \
    PGN_OUT="$pgn_out" \
    META_OUT="$meta_out" \
    ENGINE_LOG_OUT="$engine_log_out" \
    "$script_dir/match.sh"
)"

require_output() {
    local expected="$1"
    if [[ "$output" != *"$expected"* ]]; then
        echo "dry-run output is missing: $expected" >&2
        exit 1
    fi
}

require_output "-srand 20260829"
require_output "nodes=5000"
require_output "seldepth=true"
require_output "nps=true"
require_output "hashfull=true"
require_output "pv=true"
require_output "timeleft=true"
require_output "latency=true"
require_output "timemargin=0"
require_output "option.Move\\ Overhead=30"
require_output "option.EvalMix=100"
require_output "option.EvalMix=10"
require_output "-show-latency"
require_output "-strict"
require_output "-autosaveinterval 2"
require_output "outname=$scratch/smoke.config.json"
require_output "-log file=$engine_log_out level=trace engine=true append=false realtime=true"

if [[ ! -f "$meta_out" ]]; then
    echo "metadata file was not created" >&2
    exit 1
fi

for expected in \
    "engine_a_git_sha=candidate-sha" \
    "engine_b_git_sha=baseline-sha" \
    "engine_a_eval_mix=100" \
    "engine_b_eval_mix=10" \
    "opening_seed=20260829" \
    "limit_mode=nodes" \
    "time_control=-" \
    "nodes=5000" \
    "time_margin_ms=0" \
    "move_overhead_ms=30" \
    "show_latency=1" \
    "strict=1" \
    "warning_policy=reject-all" \
    "campaign_id=campaign-identity" \
    "shard_id=shard-0007" \
    "campaign_manifest_sha256=campaign-manifest-hash" \
    "shard_manifest_sha256=shard-manifest-hash" \
    "pair_offset=14" \
    "autosave_interval=2" \
    "games=10" \
    "engine_log_out=$engine_log_out" \
    "config_out=$scratch/smoke.config.json"; do
    if ! grep -Fqx "$expected" "$meta_out"; then
        echo "metadata is missing: $expected" >&2
        exit 1
    fi
done

if DRY_RUN=1 FASTCHESS_BIN=true ENGINE_A=/usr/bin/true ENGINE_B=/usr/bin/true \
    ENGINE_A_EVAL_MIX=101 OPENINGS_FILE="$repo_root/scripts/openings.epd" \
    GAMES=10 PGN_OUT="$scratch/invalid-eval-mix.pgn" "$script_dir/match.sh" \
    >/dev/null 2>&1; then
    echo "match runner accepted an out-of-range EvalMix" >&2
    exit 1
fi

echo "match configuration test passed"

per_engine_meta_out="$scratch/per-engine.meta.txt"
per_engine_output="$(
    DRY_RUN=1 \
    FASTCHESS_BIN=true \
    ENGINE_A=/usr/bin/true \
    ENGINE_B=/usr/bin/true \
    ENGINE_A_NODES=30000 \
    ENGINE_B_NODES=29200 \
    OPENINGS_FILE="$repo_root/scripts/openings.epd" \
    GAMES=10 \
    META_OUT="$per_engine_meta_out" \
    PGN_OUT="$scratch/per-engine.pgn" \
    "$script_dir/match.sh"
)"

for expected in \
    "name=NEYRANG-new nodes=30000" \
    "name=NEYRANG-reference nodes=29200"; do
    if [[ "$per_engine_output" != *"$expected"* ]]; then
        echo "per-engine dry-run output is missing: $expected" >&2
        exit 1
    fi
done

for expected in \
    "limit_mode=per-engine-nodes" \
    "time_control=-" \
    "nodes=" \
    "engine_a_nodes=30000" \
    "engine_b_nodes=29200"; do
    if ! grep -Fqx "$expected" "$per_engine_meta_out"; then
        echo "per-engine metadata is missing: $expected" >&2
        exit 1
    fi
done

if DRY_RUN=1 FASTCHESS_BIN=true ENGINE_A=/usr/bin/true ENGINE_B=/usr/bin/true \
    ENGINE_A_NODES=30000 OPENINGS_FILE="$repo_root/scripts/openings.epd" \
    GAMES=10 PGN_OUT="$scratch/invalid.pgn" "$script_dir/match.sh" \
    >/dev/null 2>&1; then
    echo "match runner accepted an incomplete per-engine node limit" >&2
    exit 1
fi

if DRY_RUN=1 FASTCHESS_BIN=true ENGINE_A=/usr/bin/true ENGINE_B=/usr/bin/true \
    NODES=30000 ENGINE_A_NODES=30000 ENGINE_B_NODES=29200 \
    OPENINGS_FILE="$repo_root/scripts/openings.epd" GAMES=10 \
    PGN_OUT="$scratch/invalid-mixed.pgn" "$script_dir/match.sh" \
    >/dev/null 2>&1; then
    echo "match runner accepted mixed shared and per-engine node limits" >&2
    exit 1
fi

echo "per-engine node configuration test passed"

per_engine_threads_meta_out="$scratch/per-engine-threads.meta.txt"
per_engine_threads_output="$(
    DRY_RUN=1 \
    FASTCHESS_BIN=true \
    ENGINE_A=/usr/bin/true \
    ENGINE_B=/usr/bin/true \
    ENGINE_A_THREADS=2 \
    ENGINE_B_THREADS=1 \
    OPENINGS_FILE="$repo_root/scripts/openings.epd" \
    GAMES=10 \
    META_OUT="$per_engine_threads_meta_out" \
    PGN_OUT="$scratch/per-engine-threads.pgn" \
    "$script_dir/match.sh"
)"

for expected in \
    "name=NEYRANG-new option.Threads=2" \
    "name=NEYRANG-reference option.Threads=1"; do
    if [[ "$per_engine_threads_output" != *"$expected"* ]]; then
        echo "per-engine Threads dry-run output is missing: $expected" >&2
        exit 1
    fi
done

for expected in \
    "thread_mode=per-engine" \
    "threads=" \
    "engine_a_threads=2" \
    "engine_b_threads=1"; do
    if ! grep -Fqx "$expected" "$per_engine_threads_meta_out"; then
        echo "per-engine Threads metadata is missing: $expected" >&2
        exit 1
    fi
done

if DRY_RUN=1 FASTCHESS_BIN=true ENGINE_A=/usr/bin/true ENGINE_B=/usr/bin/true \
    ENGINE_A_THREADS=2 OPENINGS_FILE="$repo_root/scripts/openings.epd" \
    GAMES=10 PGN_OUT="$scratch/invalid-threads.pgn" "$script_dir/match.sh" \
    >/dev/null 2>&1; then
    echo "match runner accepted an incomplete per-engine Threads configuration" >&2
    exit 1
fi

if DRY_RUN=1 FASTCHESS_BIN=true ENGINE_A=/usr/bin/true ENGINE_B=/usr/bin/true \
    THREADS=4 ENGINE_A_THREADS=2 ENGINE_B_THREADS=1 \
    OPENINGS_FILE="$repo_root/scripts/openings.epd" GAMES=10 \
    PGN_OUT="$scratch/invalid-mixed-threads.pgn" "$script_dir/match.sh" \
    >/dev/null 2>&1; then
    echo "match runner accepted mixed shared and per-engine Threads" >&2
    exit 1
fi

echo "per-engine Threads configuration test passed"

mixed_threads_meta_out="$scratch/mixed-threads.meta.txt"
mixed_threads_output="$(
    DRY_RUN=1 \
    FASTCHESS_BIN=true \
    ENGINE_A=/usr/bin/true \
    ENGINE_B=/usr/bin/true \
    ENGINE_A_THREADS=4 \
    ENGINE_B_THREADS=default \
    OPENINGS_FILE="$repo_root/scripts/openings.epd" \
    GAMES=10 \
    META_OUT="$mixed_threads_meta_out" \
    PGN_OUT="$scratch/mixed-threads.pgn" \
    "$script_dir/match.sh"
)"

if [[ "$mixed_threads_output" != *"name=NEYRANG-new option.Threads=4"* ]] ||
    [[ "$mixed_threads_output" == *"name=NEYRANG-reference option.Threads="* ]]; then
    echo "mixed explicit/default Threads dry run is incorrect" >&2
    exit 1
fi
for expected in \
    "thread_mode=per-engine" \
    "engine_a_threads=4" \
    "engine_b_threads=default"; do
    if ! grep -Fqx "$expected" "$mixed_threads_meta_out"; then
        echo "mixed Threads metadata is missing: $expected" >&2
        exit 1
    fi
done

echo "mixed explicit/default Threads configuration test passed"

default_threads_meta_out="$scratch/default-threads.meta.txt"
default_threads_output="$(
    DRY_RUN=1 \
    FASTCHESS_BIN=true \
    ENGINE_A=/usr/bin/true \
    ENGINE_B=/usr/bin/true \
    THREADS=default \
    OPENINGS_FILE="$repo_root/scripts/openings.epd" \
    GAMES=10 \
    META_OUT="$default_threads_meta_out" \
    PGN_OUT="$scratch/default-threads.pgn" \
    "$script_dir/match.sh"
)"

if [[ "$default_threads_output" == *"option.Threads="* ]]; then
    echo "engine-default dry run unexpectedly set a Threads option" >&2
    exit 1
fi
for expected in \
    "thread_mode=engine-default" \
    "threads=default" \
    "engine_a_threads=default" \
    "engine_b_threads=default"; do
    if ! grep -Fqx "$expected" "$default_threads_meta_out"; then
        echo "engine-default Threads metadata is missing: $expected" >&2
        exit 1
    fi
done

echo "engine-default Threads configuration test passed"

network_fixture="$repo_root/scripts/openings.epd"
network_sha256="$(shasum -a 256 "$network_fixture" | awk '{print $1}')"
nnue_meta_out="$scratch/nnue.meta.txt"
nnue_output="$(
    DRY_RUN=1 \
    FASTCHESS_BIN=true \
    ENGINE_A=/usr/bin/true \
    ENGINE_B=/usr/bin/true \
    ENGINE_A_EVAL_FILE="$network_fixture" \
    OPENINGS_FILE="$repo_root/scripts/openings.epd" \
    GAMES=10 \
    META_OUT="$nnue_meta_out" \
    PGN_OUT="$scratch/nnue.pgn" \
    "$script_dir/match.sh"
)"

if [[ "$nnue_output" != *"name=NEYRANG-new option.EvalFile=$network_fixture"* ]]; then
    echo "NNUE dry-run output is missing the candidate EvalFile" >&2
    exit 1
fi
for expected in \
    "engine_a_eval_file=$network_fixture" \
    "engine_a_eval_file_sha256=$network_sha256" \
    "engine_b_eval_file=" \
    "engine_b_eval_file_sha256="; do
    if ! grep -Fqx "$expected" "$nnue_meta_out"; then
        echo "NNUE metadata is missing: $expected" >&2
        exit 1
    fi
done

echo "per-engine EvalFile configuration test passed"

policy_meta_out="$scratch/policy.meta.txt"
policy_output="$(
    DRY_RUN=1 \
    FASTCHESS_BIN=true \
    ENGINE_A=/usr/bin/true \
    ENGINE_B=/usr/bin/true \
    ENGINE_A_POLICY_FILE="$network_fixture" \
    OPENINGS_FILE="$repo_root/scripts/openings.epd" \
    GAMES=10 \
    META_OUT="$policy_meta_out" \
    PGN_OUT="$scratch/policy.pgn" \
    "$script_dir/match.sh"
)"

if [[ "$policy_output" != *"name=NEYRANG-new option.PolicyFile=$network_fixture"* ]]; then
    echo "policy dry-run output is missing the candidate PolicyFile" >&2
    exit 1
fi
for expected in \
    "engine_a_policy_file=$network_fixture" \
    "engine_a_policy_file_sha256=$network_sha256" \
    "engine_b_policy_file=" \
    "engine_b_policy_file_sha256="; do
    if ! grep -Fqx "$expected" "$policy_meta_out"; then
        echo "policy metadata is missing: $expected" >&2
        exit 1
    fi
done

echo "per-engine PolicyFile configuration test passed"

compat_meta_out="$scratch/compat.meta.txt"
compat_output="$(
    DRY_RUN=1 \
    FASTCHESS_BIN=true \
    ENGINE_A=/usr/bin/true \
    ENGINE_B=/usr/bin/true \
    OPENINGS_FILE="$repo_root/scripts/openings.epd" \
    WARNING_POLICY=allow-opponent-threefold-pv \
    STRICT=0 \
    GAMES=10 \
    META_OUT="$compat_meta_out" \
    PGN_OUT="$scratch/compat.pgn" \
    "$script_dir/match.sh"
)"

if [[ "$compat_output" == *"-strict"* ]]; then
    echo "baseline compatibility dry run unexpectedly enabled strict mode" >&2
    exit 1
fi
if ! grep -Fqx "warning_policy=allow-opponent-threefold-pv" "$compat_meta_out"; then
    echo "compatibility metadata is missing the warning policy" >&2
    exit 1
fi
if DRY_RUN=1 FASTCHESS_BIN=true ENGINE_A=/usr/bin/true ENGINE_B=/usr/bin/true \
    OPENINGS_FILE="$repo_root/scripts/openings.epd" \
    WARNING_POLICY=allow-opponent-threefold-pv STRICT=1 GAMES=10 \
    PGN_OUT="$scratch/invalid-policy.pgn" "$script_dir/match.sh" \
    >/dev/null 2>&1; then
    echo "match runner accepted strict mode with a compatibility warning policy" >&2
    exit 1
fi

echo "baseline warning-policy configuration test passed"

sprt_meta_out="$scratch/sprt.meta.txt"
sprt_output="$(
    DRY_RUN=1 \
    FASTCHESS_BIN=true \
    ENGINE_A=/usr/bin/true \
    ENGINE_B=/usr/bin/true \
    ENGINE_A_EVAL_FILE="$network_fixture" \
    ENGINE_A_GIT_SHA=candidate-sha \
    ENGINE_B_GIT_SHA=baseline-sha \
    OPENINGS_FILE="$repo_root/scripts/openings.epd" \
    OPENING_SEED=20260829 \
    TIME_MARGIN_MS=0 \
    MOVE_OVERHEAD_MS=30 \
    SHOW_LATENCY=1 \
    STRICT=1 \
    AUTOSAVE_INTERVAL=2 \
    ROUNDS=20 \
    SPRT_MODEL=normalized \
    META_OUT="$sprt_meta_out" \
    PGN_OUT="$scratch/sprt.pgn" \
    "$script_dir/sprt.sh"
)"

for expected in \
    "-srand 20260829" \
    "model=normalized" \
    "-rounds 20" \
    "seldepth=true" \
    "timeleft=true" \
    "latency=true" \
    "timemargin=0" \
    "option.Move\\ Overhead=30" \
    "-show-latency" \
    "-strict" \
    "-autosaveinterval 2" \
    "outname=$scratch/sprt.config.json"; do
    if [[ "$sprt_output" != *"$expected"* ]]; then
        echo "SPRT dry-run output is missing: $expected" >&2
        exit 1
    fi
done

if [[ "$sprt_output" != *"name=NEYRANG-candidate option.EvalFile=$network_fixture"* ]]; then
    echo "SPRT dry-run output is missing the candidate EvalFile" >&2
    exit 1
fi

if [[ ! -f "$sprt_meta_out" ]]; then
    echo "SPRT metadata file was not created" >&2
    exit 1
fi

for expected in \
    "engine_a_git_sha=candidate-sha" \
    "engine_b_git_sha=baseline-sha" \
    "engine_a_eval_file=$network_fixture" \
    "engine_a_eval_file_sha256=$network_sha256" \
    "opening_seed=20260829" \
    "sprt_model=normalized" \
    "rounds=20" \
    "time_margin_ms=0" \
    "move_overhead_ms=30" \
    "show_latency=1" \
    "strict=1" \
    "autosave_interval=2" \
    "config_out=$scratch/sprt.config.json"; do
    if ! grep -Fqx "$expected" "$sprt_meta_out"; then
        echo "SPRT metadata is missing: $expected" >&2
        exit 1
    fi
done

echo "SPRT configuration test passed"
