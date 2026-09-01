#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd -- "$script_dir/.." && pwd)"
release_binary="$repo_root/target/release/neyrang"
artifact_dir="$repo_root/dist/en-croissant"
default_pgn="$repo_root/examples/legacy/neyrang-vs-stockfish-smoke.pgn"

usage() {
    cat <<'EOF'
Usage: scripts/en-croissant.sh <command> [PGN]

Commands:
  prepare       Build NEYRANG and create a stable executable for En Croissant
  path          Print the prepared engine path (prepares it when missing)
  open [PGN]    Open a PGN in En Croissant (defaults to the legacy smoke games)
  help          Show this message
EOF
}

require_macos() {
    if [[ "$(uname -s)" != "Darwin" ]]; then
        echo "This helper currently supports the macOS build of En Croissant." >&2
        exit 2
    fi
}

prepare_engine() {
    require_macos
    cargo build --manifest-path "$repo_root/Cargo.toml" --release

    local version architecture artifact
    version="$("$release_binary" --version | awk '{print $2}')"
    architecture="$(uname -m)"
    artifact="$artifact_dir/neyrang-$version-macos-$architecture"

    mkdir -p "$artifact_dir"
    install -m 755 "$release_binary" "$artifact"
    printf '%s\n' "$artifact"
}

engine_path() {
    local candidate
    candidate="$(find "$artifact_dir" -maxdepth 1 -type f -name 'neyrang-*-macos-*' -print 2>/dev/null | sort | tail -n 1)"
    if [[ -z "$candidate" || ! -x "$candidate" ]]; then
        prepare_engine
    else
        printf '%s\n' "$candidate"
    fi
}

open_pgn() {
    require_macos
    local pgn="${1:-$default_pgn}"
    local app="/Applications/en-croissant.app"

    if [[ ! -f "$pgn" ]]; then
        echo "PGN file not found: $pgn" >&2
        exit 2
    fi
    if [[ ! -d "$app" ]]; then
        echo "En Croissant was not found at $app" >&2
        exit 2
    fi

    open -a "$app" "$pgn"
}

case "${1:-help}" in
    prepare)
        prepare_engine
        ;;
    path)
        engine_path
        ;;
    open)
        open_pgn "${2:-}"
        ;;
    help | -h | --help)
        usage
        ;;
    *)
        usage >&2
        exit 2
        ;;
esac
