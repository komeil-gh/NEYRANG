#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd -- "$script_dir/.." && pwd)"
engine="${ENGINE:-$repo_root/target/release/neyrang}"
runs="${RUNS:-5}"
depth="${DEPTH:-5}"

if [[ ! -x "$engine" ]]; then
    echo "Release engine not found; run cargo build --release first" >&2
    exit 2
fi

times=()
for ((run = 1; run <= runs; run++)); do
    output="$($engine bench "$depth")"
    echo "run $run"
    echo "$output"
    milliseconds="$(awk '/^time:/ { print $2 }' <<<"$output")"
    times+=("$milliseconds")
done

sorted="$(printf '%s\n' "${times[@]}" | sort -n)"
median="$(awk -v count="$runs" 'NR == int((count + 1) / 2) { print; exit }' <<<"$sorted")"
echo "median time: $median ms"
