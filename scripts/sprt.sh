#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd -- "$script_dir/.." && pwd)"

fastchess_bin="${FASTCHESS_BIN:-fastchess}"
candidate="${ENGINE_A:-$repo_root/target/release/neyrang}"
baseline="${ENGINE_B:-}"
candidate_name="${ENGINE_A_NAME:-NEYRANG-candidate}"
baseline_name="${ENGINE_B_NAME:-NEYRANG-parent}"
rounds="${ROUNDS:-100000}"
concurrency="${CONCURRENCY:-1}"
time_control="${TC:-10+0.1}"
hash_mb="${HASH_MB:-64}"
threads="${THREADS:-1}"
elo0="${ELO0:-0}"
elo1="${ELO1:-5}"
alpha="${ALPHA:-0.05}"
beta="${BETA:-0.05}"
sprt_model="${SPRT_MODEL:-normalized}"
openings_file="${OPENINGS_FILE:-$script_dir/openings.epd}"
opening_order="${OPENING_ORDER:-random}"
opening_seed="${OPENING_SEED:-20260829}"
pgn_out="${PGN_OUT:-$repo_root/results/sprt.pgn}"
meta_out="${META_OUT:-${pgn_out%.pgn}.meta.txt}"
log_out="${LOG_OUT:-${pgn_out%.pgn}.log}"
config_out="${CONFIG_OUT:-${pgn_out%.pgn}.config.json}"
candidate_git_sha="${ENGINE_A_GIT_SHA:-}"
baseline_git_sha="${ENGINE_B_GIT_SHA:-unknown}"
openings_source="${OPENINGS_SOURCE:-local}"
openings_license="${OPENINGS_LICENSE:-unknown}"
dry_run="${DRY_RUN:-0}"

if [[ -z "$baseline" ]]; then
    echo "ENGINE_B must point to the parent UCI engine" >&2
    exit 2
fi
if [[ ! -x "$candidate" || ! -x "$baseline" ]]; then
    echo "Both ENGINE_A and ENGINE_B must be executable" >&2
    exit 2
fi
if [[ "$fastchess_bin" == */* ]]; then
    fastchess_path="$fastchess_bin"
else
    fastchess_path="$(type -P "$fastchess_bin" || true)"
fi
if [[ -z "$fastchess_path" || ! -x "$fastchess_path" ]]; then
    echo "fastchess executable not found: $fastchess_bin" >&2
    exit 2
fi
if (( rounds < 1 )); then
    echo "ROUNDS must be positive" >&2
    exit 2
fi
if [[ "$opening_order" != "random" && "$opening_order" != "sequential" ]]; then
    echo "OPENING_ORDER must be random or sequential" >&2
    exit 2
fi

if [[ -z "$candidate_git_sha" ]]; then
    candidate_git_sha="$(git -C "$repo_root" rev-parse HEAD 2>/dev/null || printf 'unknown')"
fi

mkdir -p \
    "$(dirname -- "$pgn_out")" \
    "$(dirname -- "$meta_out")" \
    "$(dirname -- "$log_out")" \
    "$(dirname -- "$config_out")"

sha256_file() {
    shasum -a 256 "$1" | awk '{print $1}'
}

fastchess_version="$("$fastchess_path" --version 2>/dev/null || true)"
fastchess_version="${fastchess_version:-unknown}"
candidate_sha256="$(sha256_file "$candidate")"
baseline_sha256="$(sha256_file "$baseline")"
openings_sha256="$(sha256_file "$openings_file")"
fastchess_sha256="$(sha256_file "$fastchess_path")"

command=(
    "$fastchess_path"
    -engine "cmd=$candidate" "name=$candidate_name"
    -engine "cmd=$baseline" "name=$baseline_name"
    -each "tc=$time_control" "option.Hash=$hash_mb" "option.Threads=$threads"
    -openings "file=$openings_file" format=epd "order=$opening_order"
    -srand "$opening_seed"
    -sprt "elo0=$elo0" "elo1=$elo1" "alpha=$alpha" "beta=$beta" "model=$sprt_model"
    -rounds "$rounds" -repeat
    -concurrency "$concurrency"
    -pgnout "file=$pgn_out" notation=san append=false nodes=true seldepth=true nps=true hashfull=true pv=true timeleft=true
    -report penta=true
    -ratinginterval 10
    -config "outname=$config_out"
    -recover
)

{
    echo "format=neyrang-sprt-v1"
    echo "created_utc=$(date -u +%Y-%m-%dT%H:%M:%SZ)"
    echo "engine_a=$candidate"
    echo "engine_a_name=$candidate_name"
    echo "engine_a_git_sha=$candidate_git_sha"
    echo "engine_a_sha256=$candidate_sha256"
    echo "engine_b=$baseline"
    echo "engine_b_name=$baseline_name"
    echo "engine_b_git_sha=$baseline_git_sha"
    echo "engine_b_sha256=$baseline_sha256"
    echo "fastchess=$fastchess_path"
    echo "fastchess_version=$fastchess_version"
    echo "fastchess_sha256=$fastchess_sha256"
    echo "openings_file=$openings_file"
    echo "openings_sha256=$openings_sha256"
    echo "openings_source=$openings_source"
    echo "openings_license=$openings_license"
    echo "opening_order=$opening_order"
    echo "opening_seed=$opening_seed"
    echo "rounds=$rounds"
    echo "maximum_games=$((rounds * 2))"
    echo "time_control=$time_control"
    echo "threads=$threads"
    echo "hash_mb=$hash_mb"
    echo "concurrency=$concurrency"
    echo "sprt_elo0=$elo0"
    echo "sprt_elo1=$elo1"
    echo "sprt_alpha=$alpha"
    echo "sprt_beta=$beta"
    echo "sprt_model=$sprt_model"
    echo "adjudication=fastchess-default"
    echo "pgn_out=$pgn_out"
    echo "log_out=$log_out"
    echo "config_out=$config_out"
} >"$meta_out"

if [[ "$dry_run" == "1" ]]; then
    printf 'metadata=%s\n' "$meta_out"
    printf '%q ' "${command[@]}"
    printf '\n'
    exit 0
fi

"${command[@]}" 2>&1 | tee "$log_out"
{
    echo "completed_utc=$(date -u +%Y-%m-%dT%H:%M:%SZ)"
    echo "status=completed"
} >>"$meta_out"
