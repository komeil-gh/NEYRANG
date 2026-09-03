#!/usr/bin/env python3
"""Build a deterministic, annotation-free Bobby Fischer style corpus."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import shutil
import sys
from collections import Counter, defaultdict
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

import chess
import chess.pgn

SCHEMA = "neyrang-fischer-corpus-v1"
PARTITIONS = ("train", "validation", "holdout")
ALIASES = {
    "bobby fischer",
    "fischer bobby",
    "fischer robert james",
    "robert james fischer",
}
SOURCE_ID = re.compile(r"^[A-Za-z0-9][A-Za-z0-9._-]*$")
HEADER_ALLOWLIST = ("Event", "Site", "Date", "Round", "White", "Black", "Result", "ECO")


class CorpusError(RuntimeError):
    """A fail-closed corpus validation error."""


@dataclass(frozen=True)
class Source:
    source_id: str
    path: Path
    url: str | None


@dataclass
class GameRecord:
    game_id: str
    source_ids: list[str]
    headers: dict[str, str]
    initial_fen: str
    moves: list[str]
    fischer_color: str
    decisions: list[dict[str, Any]]


class UnionFind:
    def __init__(self, size: int) -> None:
        self.parent = list(range(size))

    def find(self, item: int) -> int:
        while self.parent[item] != item:
            self.parent[item] = self.parent[self.parent[item]]
            item = self.parent[item]
        return item

    def union(self, left: int, right: int) -> None:
        left_root = self.find(left)
        right_root = self.find(right)
        if left_root != right_root:
            self.parent[right_root] = left_root


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--source",
        action="append",
        required=True,
        metavar="ID=PGN",
        help="PGN source; repeat to merge and deduplicate collections",
    )
    parser.add_argument(
        "--source-url",
        action="append",
        default=[],
        metavar="ID=URL",
        help="public provenance URL for a source ID",
    )
    parser.add_argument("--output-dir", required=True, type=Path)
    parser.add_argument("--seed", type=int, default=2_026_090_301)
    parser.add_argument("--min-ply", type=int, default=12)
    parser.add_argument("--train-percent", type=int, default=80)
    parser.add_argument("--validation-percent", type=int, default=10)
    return parser.parse_args()


def parse_mapping(items: list[str], label: str) -> dict[str, str]:
    values: dict[str, str] = {}
    for item in items:
        key, separator, value = item.partition("=")
        if not separator or not SOURCE_ID.fullmatch(key) or not value:
            raise CorpusError(f"invalid {label}: {item!r}")
        if key in values:
            raise CorpusError(f"duplicate {label} ID: {key}")
        values[key] = value
    return values


def normalize_name(value: str) -> str:
    return " ".join(re.sub(r"[^a-z]+", " ", value.casefold()).split())


def is_fischer(value: str) -> bool:
    return normalize_name(value) in ALIASES


def sha256_bytes(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def canonical_position(board: chess.Board) -> str:
    return " ".join(board.fen(en_passant="legal").split()[:4])


def game_identity(initial_fen: str, moves: list[str]) -> str:
    payload = (initial_fen + "\n" + " ".join(moves) + "\n").encode("ascii")
    return sha256_bytes(payload)


def read_source(source: Source, min_ply: int) -> tuple[list[GameRecord], dict[str, Any]]:
    raw = source.path.read_bytes()
    games: list[GameRecord] = []
    rejected: Counter[str] = Counter()
    with source.path.open(encoding="utf-8-sig", errors="strict") as stream:
        game_number = 0
        while True:
            game = chess.pgn.read_game(stream)
            if game is None:
                break
            game_number += 1
            if game.errors:
                raise CorpusError(
                    f"{source.source_id} game {game_number} has PGN errors: {game.errors[0]}"
                )
            variant = game.headers.get("Variant", "Standard").casefold()
            if variant not in {"standard", "chess"}:
                rejected["non_standard_variant"] += 1
                continue
            white_is_fischer = is_fischer(game.headers.get("White", ""))
            black_is_fischer = is_fischer(game.headers.get("Black", ""))
            if white_is_fischer == black_is_fischer:
                rejected["player_identity"] += 1
                continue
            result = game.headers.get("Result", "*")
            if result not in {"1-0", "0-1", "1/2-1/2"}:
                rejected["unknown_result"] += 1
                continue

            board = game.board()
            initial_fen = board.fen(en_passant="legal")
            moves: list[str] = []
            decisions: list[dict[str, Any]] = []
            fischer_color = chess.WHITE if white_is_fischer else chess.BLACK
            for ply, move in enumerate(game.mainline_moves()):
                if move not in board.legal_moves:
                    raise CorpusError(
                        f"{source.source_id} game {game_number} has illegal move {move.uci()}"
                    )
                if board.turn == fischer_color and ply >= min_ply:
                    legal_count = board.legal_moves.count()
                    if legal_count > 1:
                        position = canonical_position(board)
                        decisions.append(
                            {
                                "ply": ply,
                                "fen": board.fen(en_passant="legal"),
                                "position_key": sha256_bytes(position.encode("ascii")),
                                "move": move.uci(),
                                "legal_moves": legal_count,
                            }
                        )
                moves.append(move.uci())
                board.push(move)
            if not moves:
                rejected["empty_game"] += 1
                continue

            game_id = game_identity(initial_fen, moves)
            games.append(
                GameRecord(
                    game_id=game_id,
                    source_ids=[source.source_id],
                    headers={name: game.headers.get(name, "?") for name in HEADER_ALLOWLIST},
                    initial_fen=initial_fen,
                    moves=moves,
                    fischer_color="white" if fischer_color == chess.WHITE else "black",
                    decisions=decisions,
                )
            )
    return games, {
        "id": source.source_id,
        "path": str(source.path),
        "url": source.url,
        "bytes": len(raw),
        "sha256": sha256_bytes(raw),
        "games_read": game_number,
        "games_accepted_before_deduplication": len(games),
        "rejected": dict(sorted(rejected.items())),
    }


def deduplicate(games: list[GameRecord]) -> tuple[list[GameRecord], int]:
    by_id: dict[str, GameRecord] = {}
    duplicates = 0
    for game in games:
        existing = by_id.get(game.game_id)
        if existing is None:
            by_id[game.game_id] = game
            continue
        duplicates += 1
        existing.source_ids = sorted(set(existing.source_ids + game.source_ids))
    return [by_id[key] for key in sorted(by_id)], duplicates


def partition_games(
    games: list[GameRecord], seed: int, train_percent: int, validation_percent: int
) -> dict[str, list[GameRecord]]:
    union_find = UnionFind(len(games))
    first_owner: dict[str, int] = {}
    for index, game in enumerate(games):
        for decision in game.decisions:
            key = decision["position_key"]
            owner = first_owner.setdefault(key, index)
            union_find.union(index, owner)

    components: dict[int, list[GameRecord]] = defaultdict(list)
    for index, game in enumerate(games):
        components[union_find.find(index)].append(game)

    partitions = {name: [] for name in PARTITIONS}
    train_cutoff = train_percent * 100
    validation_cutoff = (train_percent + validation_percent) * 100
    for component in components.values():
        identity = "\n".join(sorted(game.game_id for game in component))
        bucket = int.from_bytes(
            hashlib.sha256(f"{seed}\n{identity}".encode("ascii")).digest()[:8], "big"
        ) % 10_000
        partition = (
            "train"
            if bucket < train_cutoff
            else "validation"
            if bucket < validation_cutoff
            else "holdout"
        )
        partitions[partition].extend(component)
    for partition in PARTITIONS:
        partitions[partition].sort(key=lambda game: game.game_id)
    return partitions


def write_jsonl(path: Path, records: list[dict[str, Any]]) -> dict[str, Any]:
    digest = hashlib.sha256()
    with path.open("wb") as stream:
        for record in records:
            line = (json.dumps(record, sort_keys=True, separators=(",", ":")) + "\n").encode()
            stream.write(line)
            digest.update(line)
    return {"path": path.name, "records": len(records), "bytes": path.stat().st_size, "sha256": digest.hexdigest()}


def build(args: argparse.Namespace) -> dict[str, Any]:
    if args.min_ply < 0:
        raise CorpusError("min-ply must be non-negative")
    if not 0 < args.train_percent < 100:
        raise CorpusError("train-percent must be between 1 and 99")
    if not 0 < args.validation_percent < 100 - args.train_percent:
        raise CorpusError("validation-percent leaves no holdout")

    paths = parse_mapping(args.source, "source")
    urls = parse_mapping(args.source_url, "source URL")
    unknown_urls = sorted(set(urls) - set(paths))
    if unknown_urls:
        raise CorpusError(f"source URL has no matching source: {unknown_urls[0]}")
    sources = [Source(key, Path(value).resolve(), urls.get(key)) for key, value in paths.items()]
    for source in sources:
        if not source.path.is_file():
            raise CorpusError(f"source is not a file: {source.path}")

    output = args.output_dir.resolve()
    if output.exists():
        raise CorpusError(f"refusing to reuse output directory: {output}")
    temporary = output.with_name(f".{output.name}.tmp-{os.getpid()}")
    if temporary.exists():
        raise CorpusError(f"temporary output already exists: {temporary}")

    source_manifests: list[dict[str, Any]] = []
    all_games: list[GameRecord] = []
    for source in sources:
        games, manifest = read_source(source, args.min_ply)
        all_games.extend(games)
        source_manifests.append(manifest)
    games, duplicate_games = deduplicate(all_games)
    if not games:
        raise CorpusError("no Fischer games survived validation")
    partitions = partition_games(
        games, args.seed, args.train_percent, args.validation_percent
    )

    temporary.mkdir(parents=True)
    try:
        artifacts: dict[str, dict[str, Any]] = {}
        position_partitions: dict[str, str] = {}
        for partition, partition_games_list in partitions.items():
            game_rows: list[dict[str, Any]] = []
            decision_rows: list[dict[str, Any]] = []
            for game in partition_games_list:
                game_rows.append(
                    {
                        "game_id": game.game_id,
                        "source_ids": game.source_ids,
                        "headers": game.headers,
                        "initial_fen": game.initial_fen,
                        "moves": game.moves,
                        "fischer_color": game.fischer_color,
                    }
                )
                for decision in game.decisions:
                    key = decision["position_key"]
                    previous = position_partitions.setdefault(key, partition)
                    if previous != partition:
                        raise CorpusError(f"position leakage between {previous} and {partition}")
                    decision_rows.append(
                        {
                            **decision,
                            "game_id": game.game_id,
                            "source_ids": game.source_ids,
                            "result": game.headers["Result"],
                            "fischer_color": game.fischer_color,
                        }
                    )
            artifacts[f"games_{partition}"] = write_jsonl(
                temporary / f"games-{partition}.jsonl", game_rows
            )
            artifacts[f"decisions_{partition}"] = write_jsonl(
                temporary / f"decisions-{partition}.jsonl", decision_rows
            )

        manifest = {
            "schema": SCHEMA,
            "created_at": datetime.now(timezone.utc).isoformat(),
            "policy": {
                "annotation_free": True,
                "mainline_only": True,
                "player_aliases": sorted(ALIASES),
                "min_ply": args.min_ply,
                "forced_moves_excluded": True,
                "partition_seed": args.seed,
                "partition_percentages": {
                    "train": args.train_percent,
                    "validation": args.validation_percent,
                    "holdout": 100 - args.train_percent - args.validation_percent,
                },
                "shared_position_components_are_atomic": True,
            },
            "sources": source_manifests,
            "games": {
                "accepted": len(games),
                "duplicates_removed": duplicate_games,
                "partitions": {name: len(partitions[name]) for name in PARTITIONS},
            },
            "decisions": {
                "accepted": sum(len(game.decisions) for game in games),
                "partitions": {
                    name: sum(len(game.decisions) for game in partitions[name])
                    for name in PARTITIONS
                },
            },
            "artifacts": artifacts,
        }
        manifest_path = temporary / "manifest.json"
        manifest_path.write_text(json.dumps(manifest, indent=2, sort_keys=True) + "\n")
        temporary.replace(output)
        return manifest
    except BaseException:
        shutil.rmtree(temporary, ignore_errors=True)
        raise


def main() -> int:
    args = parse_args()
    try:
        manifest = build(args)
    except (CorpusError, OSError, UnicodeError) as error:
        print(f"build-fischer-corpus: {error}", file=sys.stderr)
        return 1
    print(
        f"built {manifest['games']['accepted']} games and "
        f"{manifest['decisions']['accepted']} Fischer decisions"
    )
    print(f"manifest: {args.output_dir / 'manifest.json'}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
