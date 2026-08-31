#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
expected_bench="${OPENBENCH_EXPECTED_BENCH:-180591}"
profile="${OPENBENCH_TEST_PROFILE:-release}"
exe_name="neyrang-openbench-contract-$$"
engine="$root/$exe_name"
parallel_dir=""

cleanup() {
    rm -f "$engine"
    if [[ -n "$parallel_dir" ]]; then
        rm -rf -- "$parallel_dir"
    fi
}
trap cleanup EXIT INT TERM

make -C "$root" EXE="$exe_name" PROFILE="$profile"

if [[ ! -x "$engine" ]]; then
    echo "OpenBench build did not create executable: $engine" >&2
    exit 1
fi

first_nodes=""
for run in 1 2 3; do
    output="$($engine bench)"
    nodes="$(awk '/^nodes: [0-9]+$/ { print $2 }' <<<"$output")"
    nps="$(awk '/^nps: [0-9]+$/ { print $2 }' <<<"$output")"

    if [[ -z "$nodes" || -z "$nps" || "$nps" -le 0 ]]; then
        echo "OpenBench could not parse nodes/NPS on run $run" >&2
        exit 1
    fi

    if [[ "$nodes" != "$expected_bench" ]]; then
        echo "Wrong deterministic bench: expected $expected_bench, got $nodes" >&2
        exit 1
    fi

    if [[ -n "$first_nodes" && "$nodes" != "$first_nodes" ]]; then
        echo "Non-deterministic bench: $first_nodes then $nodes" >&2
        exit 1
    fi
    first_nodes="$nodes"
done

parallel_dir="$(mktemp -d "${TMPDIR:-/tmp}/neyrang-openbench-contract.XXXXXX")"
parallel_pids=()

for run in 1 2 3; do
    "$engine" bench >"$parallel_dir/bench-$run.txt" &
    parallel_pids+=("$!")
done

for pid in "${parallel_pids[@]}"; do
    wait "$pid"
done

for run in 1 2 3; do
    output="$(<"$parallel_dir/bench-$run.txt")"
    nodes="$(awk '/^nodes: [0-9]+$/ { print $2 }' <<<"$output")"
    nps="$(awk '/^nps: [0-9]+$/ { print $2 }' <<<"$output")"

    if [[ "$nodes" != "$expected_bench" || -z "$nps" || "$nps" -le 0 ]]; then
        echo "Concurrent OpenBench bench $run failed: nodes=${nodes:-missing} nps=${nps:-missing}" >&2
        exit 1
    fi
done

uci_output="$(printf 'uci\nisready\nquit\n' | "$engine")"

grep -Fq "option name Hash type spin" <<<"$uci_output"
grep -Fq "option name Threads type spin" <<<"$uci_output"
grep -Fxq "uciok" <<<"$uci_output"
grep -Fxq "readyok" <<<"$uci_output"

genfens_command="genfens 8 seed 7 book None"
genfens_first="$("$engine" "$genfens_command" quit)"
genfens_second="$("$engine" "$genfens_command" quit)"
genfens_high_seed="$("$engine" "genfens 8 seed 4294967303 book None" quit)"

if [[ "$genfens_first" != "$genfens_second" ]]; then
    echo "Non-deterministic genfens output for the same 64-bit seed" >&2
    exit 1
fi
if [[ "$genfens_first" == "$genfens_high_seed" ]]; then
    echo "Genfens discarded the upper 32 seed bits" >&2
    exit 1
fi
genfens_lines="$(awk '$1 == "info" && $2 == "string" && $3 == "genfens" && NF == 9 { count++ } END { print count + 0 }' <<<"$genfens_first")"
if [[ "$genfens_lines" != "8" ]]; then
    echo "Genfens emitted $genfens_lines valid lines, expected 8" >&2
    exit 1
fi

echo "OpenBench contract passed: sequential+concurrent bench=$first_nodes genfens=8 profile=$profile"
