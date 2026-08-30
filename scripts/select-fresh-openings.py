#!/usr/bin/env python3
"""Select a deterministic disjoint subset from an EPD opening book."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Any

import chess
import chess.pgn


class SelectionError(RuntimeError):
    """A fail-closed opening selection error."""


@dataclass(frozen=True)
class SelectionConfig:
    repo_root: Path
    book: Path
    excluded_pgns: tuple[Path, ...]
    excluded_epds: tuple[Path, ...]
    output: Path
    seed: str
    count: int


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--book", required=True, type=Path)
    parser.add_argument("--exclude-pgn", action="append", default=[], type=Path)
    parser.add_argument("--exclude-epd", action="append", default=[], type=Path)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--seed", required=True)
    parser.add_argument("--count", required=True, type=int)
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    repo_root = Path(__file__).resolve().parent.parent
    try:
        config = SelectionConfig(
            repo_root=repo_root,
            book=resolve_input(args.book, repo_root, "book"),
            excluded_pgns=tuple(
                resolve_input(path, repo_root, "excluded PGN")
                for path in args.exclude_pgn
            ),
            excluded_epds=tuple(
                resolve_input(path, repo_root, "excluded EPD")
                for path in args.exclude_epd
            ),
            output=resolve_output(args.output, repo_root),
            seed=args.seed,
            count=args.count,
        )
        summary = select_openings(config)
    except (SelectionError, OSError) as error:
        print(f"select-fresh-openings: {error}", file=sys.stderr)
        return 1
    print(json.dumps(summary, indent=2, sort_keys=True))
    return 0


def select_openings(config: SelectionConfig) -> dict[str, Any]:
    if not config.seed:
        raise SelectionError("seed must not be empty")
    if config.count <= 0:
        raise SelectionError("count must be positive")
    if config.output.exists():
        raise SelectionError(f"refusing to overwrite existing output: {relative(config.output, config.repo_root)}")

    book_lines, book_duplicates = read_epd(config.book)
    excluded: set[str] = set()
    exclusion_entries: list[dict[str, Any]] = []
    for path in sorted(set(config.excluded_pgns)):
        keys, games = read_pgn_openings(path)
        excluded.update(keys)
        digest, byte_count = sha256_file(path)
        exclusion_entries.append(
            {
                "kind": "pgn",
                "path": relative(path, config.repo_root),
                "sha256": digest,
                "bytes": byte_count,
                "games": games,
                "unique_openings": len(keys),
            }
        )
    for path in sorted(set(config.excluded_epds)):
        lines, duplicates = read_epd(path)
        excluded.update(lines)
        digest, byte_count = sha256_file(path)
        exclusion_entries.append(
            {
                "kind": "epd",
                "path": relative(path, config.repo_root),
                "sha256": digest,
                "bytes": byte_count,
                "lines": len(lines) + duplicates,
                "duplicates": duplicates,
                "unique_openings": len(lines),
            }
        )

    eligible = sorted(set(book_lines) - excluded)
    if len(eligible) < config.count:
        raise SelectionError(
            f"requested {config.count} openings but only {len(eligible)} remain after exclusions"
        )
    ranked = sorted(
        eligible,
        key=lambda key: (hashlib.sha256(f"{config.seed}\t{key}".encode()).digest(), key),
    )
    selected = ranked[: config.count]
    write_output(config.output, [book_lines[key] for key in selected])

    output_hash, output_bytes = sha256_file(config.output)
    book_hash, book_bytes = sha256_file(config.book)
    return {
        "seed": config.seed,
        "count": config.count,
        "ranking": "SHA-256(seed + TAB + canonical first-four-field FEN), then FEN",
        "book": {
            "path": relative(config.book, config.repo_root),
            "sha256": book_hash,
            "bytes": book_bytes,
            "lines": len(book_lines) + book_duplicates,
            "duplicates": book_duplicates,
            "unique_openings": len(book_lines),
        },
        "exclusions": exclusion_entries,
        "counts": {
            "unique_excluded": len(excluded),
            "book_openings_excluded": len(set(book_lines) & excluded),
            "eligible": len(eligible),
            "selected": len(selected),
            "unselected_eligible": len(eligible) - len(selected),
        },
        "output": {
            "path": relative(config.output, config.repo_root),
            "sha256": output_hash,
            "bytes": output_bytes,
            "lines": len(selected),
            "unique_openings": len({canonical_fen(line) for line in selected}),
        },
    }


def read_epd(path: Path) -> tuple[dict[str, str], int]:
    lines: dict[str, str] = {}
    duplicates = 0
    with path.open(encoding="utf-8") as handle:
        for line_number, raw in enumerate(handle, start=1):
            line = raw.strip()
            if not line:
                continue
            key = canonical_fen(line)
            validate_fen(key, path, line_number)
            if key in lines:
                duplicates += 1
                continue
            lines[key] = line
    if not lines:
        raise SelectionError(f"EPD contains no openings: {path}")
    return lines, duplicates


def read_pgn_openings(path: Path) -> tuple[set[str], int]:
    openings: set[str] = set()
    games = 0
    with path.open(encoding="utf-8") as handle:
        while game := chess.pgn.read_game(handle):
            games += 1
            if game.errors:
                raise SelectionError(f"PGN parse errors in {path} game {games}: {game.errors}")
            fen = game.headers.get("FEN", chess.STARTING_FEN)
            key = canonical_fen(fen)
            validate_fen(key, path, games)
            openings.add(key)
    if games == 0:
        raise SelectionError(f"PGN contains no games: {path}")
    return openings, games


def canonical_fen(fen: str) -> str:
    fields = fen.split()
    if len(fields) < 4:
        raise SelectionError(f"FEN has fewer than four fields: {fen!r}")
    return " ".join(fields[:4])


def validate_fen(key: str, path: Path, index: int) -> None:
    try:
        chess.Board(f"{key} 0 1")
    except ValueError as error:
        raise SelectionError(f"invalid FEN in {path} item {index}: {error}") from error


def write_output(path: Path, lines: list[str]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_name(f".{path.name}.tmp-{os.getpid()}")
    if temporary.exists():
        raise SelectionError(f"temporary output already exists: {temporary}")
    try:
        with temporary.open("x", encoding="utf-8", newline="\n") as handle:
            for line in lines:
                handle.write(line)
                handle.write("\n")
        temporary.replace(path)
    except BaseException:
        temporary.unlink(missing_ok=True)
        raise


def resolve_input(path: Path, repo_root: Path, label: str) -> Path:
    resolved = inside_repo(path, repo_root, label)
    if not resolved.is_file():
        raise SelectionError(f"{label} is not a file: {path}")
    return resolved


def resolve_output(path: Path, repo_root: Path) -> Path:
    return inside_repo(path, repo_root, "output")


def inside_repo(path: Path, repo_root: Path, label: str) -> Path:
    candidate = path if path.is_absolute() else repo_root / path
    resolved = candidate.resolve()
    try:
        resolved.relative_to(repo_root.resolve())
    except ValueError as error:
        raise SelectionError(f"{label} must be inside the repository") from error
    return resolved


def relative(path: Path, repo_root: Path) -> str:
    try:
        return path.resolve().relative_to(repo_root.resolve()).as_posix()
    except ValueError as error:
        raise SelectionError(f"path is outside repository: {path}") from error


def sha256_file(path: Path) -> tuple[str, int]:
    digest = hashlib.sha256()
    byte_count = 0
    with path.open("rb") as handle:
        while chunk := handle.read(1024 * 1024):
            digest.update(chunk)
            byte_count += len(chunk)
    return digest.hexdigest(), byte_count


if __name__ == "__main__":
    raise SystemExit(main())
