#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd -- "$script_dir/.." && pwd)"

fastchess_bin="${FASTCHESS_BIN:-fastchess}"
candidate="${ENGINE_A:-$repo_root/target/release/neyrang}"
baseline="${ENGINE_B:-}"
rounds="${ROUNDS:-100000}"
concurrency="${CONCURRENCY:-1}"
time_control="${TC:-10+0.1}"
hash_mb="${HASH_MB:-64}"
threads="${THREADS:-1}"
elo0="${ELO0:-0}"
elo1="${ELO1:-5}"
alpha="${ALPHA:-0.05}"
beta="${BETA:-0.05}"
openings_file="${OPENINGS_FILE:-$script_dir/openings.epd}"
pgn_out="${PGN_OUT:-$repo_root/results/sprt.pgn}"

if [[ -z "$baseline" ]]; then
    echo "ENGINE_B must point to the parent UCI engine" >&2
    exit 2
fi
if [[ ! -x "$candidate" || ! -x "$baseline" ]]; then
    echo "Both ENGINE_A and ENGINE_B must be executable" >&2
    exit 2
fi
if ! command -v "$fastchess_bin" >/dev/null 2>&1; then
    echo "fastchess executable not found: $fastchess_bin" >&2
    exit 2
fi

mkdir -p "$(dirname -- "$pgn_out")"

"$fastchess_bin" \
    -engine "cmd=$candidate" name=NEYRANG-candidate \
    -engine "cmd=$baseline" name=NEYRANG-parent \
    -each "tc=$time_control" "option.Hash=$hash_mb" "option.Threads=$threads" \
    -openings "file=$openings_file" format=epd order=random \
    -sprt "elo0=$elo0" "elo1=$elo1" "alpha=$alpha" "beta=$beta" model=normalized \
    -rounds "$rounds" -repeat -concurrency "$concurrency" \
    -pgnout "file=$pgn_out" notation=san append=false nodes=true \
    -recover
