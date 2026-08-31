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
nodes="${NODES:-}"
engine_a_nodes="${ENGINE_A_NODES:-}"
engine_b_nodes="${ENGINE_B_NODES:-}"
hash_mb="${HASH_MB:-64}"
threads="${THREADS:-1}"
move_overhead_ms="${MOVE_OVERHEAD_MS:-}"
time_margin_ms="${TIME_MARGIN_MS:-}"
strict="${STRICT:-0}"
warning_policy="${WARNING_POLICY:-reject-all}"
show_latency="${SHOW_LATENCY:-0}"
autosave_interval="${AUTOSAVE_INTERVAL:-20}"
openings_file="${OPENINGS_FILE:-$script_dir/openings.epd}"
opening_order="${OPENING_ORDER:-random}"
opening_seed="${OPENING_SEED:-20260829}"
pgn_out="${PGN_OUT:-$repo_root/results/match.pgn}"
meta_out="${META_OUT:-${pgn_out%.pgn}.meta.txt}"
log_out="${LOG_OUT:-${pgn_out%.pgn}.log}"
config_out="${CONFIG_OUT:-${pgn_out%.pgn}.config.json}"
engine_a_git_sha="${ENGINE_A_GIT_SHA:-}"
engine_b_git_sha="${ENGINE_B_GIT_SHA:-unknown}"
openings_source="${OPENINGS_SOURCE:-local}"
openings_license="${OPENINGS_LICENSE:-unknown}"
dry_run="${DRY_RUN:-0}"

if [[ -z "$engine_b" ]]; then
    echo "ENGINE_B must point to the reference UCI engine" >&2
    exit 2
fi
if (( games < 2 || games % 2 != 0 )); then
    echo "GAMES must be a positive even number for paired openings" >&2
    exit 2
fi
if [[ -n "$nodes" ]] && ! [[ "$nodes" =~ ^[1-9][0-9]*$ ]]; then
    echo "NODES must be a positive integer when set" >&2
    exit 2
fi
if [[ -n "$engine_a_nodes" && -z "$engine_b_nodes" ]] ||
    [[ -z "$engine_a_nodes" && -n "$engine_b_nodes" ]]; then
    echo "ENGINE_A_NODES and ENGINE_B_NODES must be set together" >&2
    exit 2
fi
if [[ -n "$nodes" && -n "$engine_a_nodes" ]]; then
    echo "NODES cannot be combined with per-engine node limits" >&2
    exit 2
fi
if [[ -n "$engine_a_nodes" ]] &&
    { ! [[ "$engine_a_nodes" =~ ^[1-9][0-9]*$ ]] ||
        ! [[ "$engine_b_nodes" =~ ^[1-9][0-9]*$ ]]; }; then
    echo "ENGINE_A_NODES and ENGINE_B_NODES must be positive integers" >&2
    exit 2
fi
if [[ -n "$time_margin_ms" ]] && ! [[ "$time_margin_ms" =~ ^[0-9]+$ ]]; then
    echo "TIME_MARGIN_MS must be a non-negative integer when set" >&2
    exit 2
fi
if [[ -n "$move_overhead_ms" ]] && ! [[ "$move_overhead_ms" =~ ^[0-9]+$ ]]; then
    echo "MOVE_OVERHEAD_MS must be a non-negative integer when set" >&2
    exit 2
fi
if [[ "$strict" != "0" && "$strict" != "1" ]]; then
    echo "STRICT must be 0 or 1" >&2
    exit 2
fi
if [[ "$warning_policy" != "reject-all" && "$warning_policy" != "allow-opponent-threefold-pv" ]]; then
    echo "WARNING_POLICY must be reject-all or allow-opponent-threefold-pv" >&2
    exit 2
fi
if [[ "$warning_policy" != "reject-all" && "$strict" != "0" ]]; then
    echo "compatibility WARNING_POLICY requires STRICT=0" >&2
    exit 2
fi
if [[ "$show_latency" != "0" && "$show_latency" != "1" ]]; then
    echo "SHOW_LATENCY must be 0 or 1" >&2
    exit 2
fi
if ! [[ "$autosave_interval" =~ ^[0-9]+$ ]]; then
    echo "AUTOSAVE_INTERVAL must be a non-negative integer" >&2
    exit 2
fi
if [[ "$opening_order" != "random" && "$opening_order" != "sequential" ]]; then
    echo "OPENING_ORDER must be random or sequential" >&2
    exit 2
fi
if [[ ! -x "$engine_a" || ! -x "$engine_b" ]]; then
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

if [[ -z "$engine_a_git_sha" ]]; then
    engine_a_git_sha="$(git -C "$repo_root" rev-parse HEAD 2>/dev/null || printf 'unknown')"
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
engine_a_sha256="$(sha256_file "$engine_a")"
engine_b_sha256="$(sha256_file "$engine_b")"
openings_sha256="$(sha256_file "$openings_file")"
fastchess_sha256="$(sha256_file "$fastchess_path")"

search_limit=("tc=$time_control")
engine_options=("option.Hash=$hash_mb" "option.Threads=$threads")
limit_mode="time"
if [[ -n "$nodes" ]]; then
    search_limit=("nodes=$nodes")
    limit_mode="nodes"
elif [[ -n "$engine_a_nodes" ]]; then
    limit_mode="per-engine-nodes"
fi
if [[ -n "$move_overhead_ms" ]]; then
    engine_options+=("option.Move Overhead=$move_overhead_ms")
fi

command=(
    "$fastchess_path"
    -engine "cmd=$engine_a" "name=$engine_a_name"
)
if [[ "$limit_mode" == "per-engine-nodes" ]]; then
    command+=("nodes=$engine_a_nodes")
fi
command+=(
    -engine "cmd=$engine_b" "name=$engine_b_name"
)
if [[ "$limit_mode" == "per-engine-nodes" ]]; then
    command+=("nodes=$engine_b_nodes")
fi
command+=(-each)
if [[ "$limit_mode" != "per-engine-nodes" ]]; then
    command+=("${search_limit[@]}")
fi
if [[ -n "$time_margin_ms" ]]; then
    command+=("timemargin=$time_margin_ms")
fi
command+=(
    "${engine_options[@]}"
    -openings "file=$openings_file" format=epd "order=$opening_order"
    -srand "$opening_seed"
    -rounds "$((games / 2))" -repeat
    -concurrency "$concurrency"
    -pgnout "file=$pgn_out" notation=san append=false nodes=true seldepth=true nps=true hashfull=true pv=true timeleft=true latency=true
    -report penta=true
    -ratinginterval 10
    -autosaveinterval "$autosave_interval"
    -config "outname=$config_out"
    -recover
)
if [[ "$show_latency" == "1" ]]; then
    command+=(-show-latency)
fi
if [[ "$strict" == "1" ]]; then
    command+=(-strict)
fi

{
    echo "format=neyrang-match-v1"
    echo "created_utc=$(date -u +%Y-%m-%dT%H:%M:%SZ)"
    echo "engine_a=$engine_a"
    echo "engine_a_name=$engine_a_name"
    echo "engine_a_git_sha=$engine_a_git_sha"
    echo "engine_a_sha256=$engine_a_sha256"
    echo "engine_b=$engine_b"
    echo "engine_b_name=$engine_b_name"
    echo "engine_b_git_sha=$engine_b_git_sha"
    echo "engine_b_sha256=$engine_b_sha256"
    echo "fastchess=$fastchess_path"
    echo "fastchess_version=$fastchess_version"
    echo "fastchess_sha256=$fastchess_sha256"
    echo "openings_file=$openings_file"
    echo "openings_sha256=$openings_sha256"
    echo "openings_source=$openings_source"
    echo "openings_license=$openings_license"
    echo "opening_order=$opening_order"
    echo "opening_seed=$opening_seed"
    echo "games=$games"
    echo "pairs=$((games / 2))"
    echo "limit_mode=$limit_mode"
    echo "time_control=$time_control"
    echo "nodes=$nodes"
    echo "engine_a_nodes=$engine_a_nodes"
    echo "engine_b_nodes=$engine_b_nodes"
    echo "threads=$threads"
    echo "hash_mb=$hash_mb"
    echo "move_overhead_ms=$move_overhead_ms"
    echo "concurrency=$concurrency"
    echo "time_margin_ms=$time_margin_ms"
    echo "show_latency=$show_latency"
    echo "strict=$strict"
    echo "warning_policy=$warning_policy"
    echo "autosave_interval=$autosave_interval"
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
