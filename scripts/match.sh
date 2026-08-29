#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd -- "$script_dir/.." && pwd)"

fastchess_bin="${FASTCHESS_BIN:-fastchess}"
engine_a="${ENGINE_A:-$repo_root/target/release/neyrang}"
engine_b="${ENGINE_B:-}"
engine_a_name="${ENGINE_A_NAME:-NEYRANG-new}"
engine_b_name="${ENGINE_B_NAME:-NEYRANG-reference}"
games="${GAMES:-200}"
concurrency="${CONCURRENCY:-1}"
time_control="${TC:-10+0.1}"
hash_mb="${HASH_MB:-64}"
threads="${THREADS:-1}"
openings_file="${OPENINGS_FILE:-$script_dir/openings.epd}"
pgn_out="${PGN_OUT:-$repo_root/results/match.pgn}"

if [[ -z "$engine_b" ]]; then
    echo "ENGINE_B must point to the reference UCI engine" >&2
    exit 2
fi
if (( games < 2 || games % 2 != 0 )); then
    echo "GAMES must be a positive even number for paired openings" >&2
    exit 2
fi
if [[ ! -x "$engine_a" || ! -x "$engine_b" ]]; then
    echo "Both ENGINE_A and ENGINE_B must be executable" >&2
    exit 2
fi
if ! command -v "$fastchess_bin" >/dev/null 2>&1; then
    echo "fastchess executable not found: $fastchess_bin" >&2
    exit 2
fi

mkdir -p "$(dirname -- "$pgn_out")"

"$fastchess_bin" \
    -engine "cmd=$engine_a" "name=$engine_a_name" \
    -engine "cmd=$engine_b" "name=$engine_b_name" \
    -each "tc=$time_control" "option.Hash=$hash_mb" "option.Threads=$threads" \
    -openings "file=$openings_file" format=epd order=sequential \
    -rounds "$((games / 2))" -repeat \
    -concurrency "$concurrency" \
    -pgnout "file=$pgn_out" notation=san append=false nodes=true \
    -ratinginterval 10 -recover
