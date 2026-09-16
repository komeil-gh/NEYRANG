#!/usr/bin/env bash
set -euo pipefail

usage() {
    echo "usage: $0 instrument|optimize PROFILE_DIR OUTPUT" >&2
    exit 2
}

[[ $# -eq 3 ]] || usage
mode="$1"
case "$mode" in instrument|optimize) ;; *) usage ;; esac
profile_dir="$(mkdir -p "$(dirname "$2")" && cd "$(dirname "$2")" && pwd)/$(basename "$2")"
output="$(mkdir -p "$(dirname "$3")" && cd "$(dirname "$3")" && pwd)/$(basename "$3")"

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd -- "$script_dir/.." && pwd)"
cargo="${CARGO:-cargo}"
rustc="${RUSTC:-rustc}"
host="$($rustc -vV | awk '/^host:/ { print $2 }')"
profdata="${LLVM_PROFDATA:-$($rustc --print sysroot)/lib/rustlib/$host/bin/llvm-profdata}"
scratch="$(mktemp -d)"
trap 'rm -rf "$scratch"' EXIT

build() {
    local rustflags="$1"
    CARGO_TARGET_DIR="$scratch/target" RUSTC="$rustc" RUSTFLAGS="$rustflags" \
        "$cargo" build --profile maxperf --locked --target "$host" \
        --manifest-path "$repo_root/Cargo.toml"
    install -m 755 "$scratch/target/$host/maxperf/neyrang" "$output"
}

[[ ! -e "$output" ]] || { echo "output already exists: $output" >&2; exit 1; }

case "$mode" in
    instrument)
        [[ ! -e "$profile_dir" ]] || { echo "profile directory already exists: $profile_dir" >&2; exit 1; }
        mkdir -p "$profile_dir/raw"
        build "-Ctarget-cpu=native -Cprofile-generate=$profile_dir/raw"
        ;;
    optimize)
        raw_profiles=("$profile_dir"/raw/*.profraw)
        [[ -x "$profdata" ]] || { echo "llvm-profdata not found: $profdata" >&2; exit 1; }
        [[ -e "${raw_profiles[0]}" ]] || { echo "no raw profiles found under: $profile_dir/raw" >&2; exit 1; }
        [[ ! -e "$profile_dir/merged.profdata" ]] || { echo "merged profile already exists" >&2; exit 1; }
        "$profdata" merge -o "$profile_dir/merged.profdata" "${raw_profiles[@]}"
        build "-Ctarget-cpu=native -Cprofile-use=$profile_dir/merged.profdata -Cllvm-args=-pgo-warn-missing-function"
        ;;
esac
