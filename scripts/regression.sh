#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd -- "$script_dir/.." && pwd)"

export ENGINE_A="${ENGINE_A:-$repo_root/target/release/neyrang}"
export ENGINE_A_NAME="${ENGINE_A_NAME:-NEYRANG-candidate}"
export ENGINE_B_NAME="${ENGINE_B_NAME:-NEYRANG-parent}"
export GAMES="${GAMES:-400}"
export PGN_OUT="${PGN_OUT:-$repo_root/results/regression.pgn}"

exec "$script_dir/match.sh"
