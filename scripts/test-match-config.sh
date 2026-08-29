#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd -- "$script_dir/.." && pwd)"
scratch="$(mktemp -d)"
trap 'rm -rf "$scratch"' EXIT

pgn_out="$scratch/smoke.pgn"
meta_out="$scratch/smoke.meta.txt"

output="$(
    DRY_RUN=1 \
    FASTCHESS_BIN=true \
    ENGINE_A=/usr/bin/true \
    ENGINE_B=/usr/bin/true \
    ENGINE_A_GIT_SHA=candidate-sha \
    ENGINE_B_GIT_SHA=baseline-sha \
    OPENINGS_FILE="$repo_root/scripts/openings.epd" \
    OPENING_SEED=20260829 \
    TIME_MARGIN_MS=0 \
    MOVE_OVERHEAD_MS=30 \
    SHOW_LATENCY=1 \
    STRICT=1 \
    AUTOSAVE_INTERVAL=2 \
    GAMES=10 \
    NODES=5000 \
    PGN_OUT="$pgn_out" \
    META_OUT="$meta_out" \
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
require_output "-show-latency"
require_output "-strict"
require_output "-autosaveinterval 2"
require_output "outname=$scratch/smoke.config.json"

if [[ ! -f "$meta_out" ]]; then
    echo "metadata file was not created" >&2
    exit 1
fi

for expected in \
    "engine_a_git_sha=candidate-sha" \
    "engine_b_git_sha=baseline-sha" \
    "opening_seed=20260829" \
    "nodes=5000" \
    "time_margin_ms=0" \
    "move_overhead_ms=30" \
    "show_latency=1" \
    "strict=1" \
    "autosave_interval=2" \
    "games=10" \
    "config_out=$scratch/smoke.config.json"; do
    if ! grep -Fqx "$expected" "$meta_out"; then
        echo "metadata is missing: $expected" >&2
        exit 1
    fi
done

echo "match configuration test passed"

sprt_meta_out="$scratch/sprt.meta.txt"
sprt_output="$(
    DRY_RUN=1 \
    FASTCHESS_BIN=true \
    ENGINE_A=/usr/bin/true \
    ENGINE_B=/usr/bin/true \
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

if [[ ! -f "$sprt_meta_out" ]]; then
    echo "SPRT metadata file was not created" >&2
    exit 1
fi

for expected in \
    "engine_a_git_sha=candidate-sha" \
    "engine_b_git_sha=baseline-sha" \
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
