#!/usr/bin/env python3
"""Deterministically merge complete NEYRANG NNUE games into one audited corpus."""

from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import os
import platform
import struct
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Any

import chess


SCHEMA = "neyrang-nnue-corpus-v1"
SHUFFLE_SCHEMA = "neyrang-nnue-game-shuffle-v1"
OPENING_DEDUP_SCHEMA = "neyrang-nnue-opening-dedup-v1"
SHARD_SCHEMA = "neyrang-nnue-selfplay-shard-v1"
CONTRACT = "neyrang-viriformat-strict-v1"
PARTITIONS = ("train", "validation")
HEADER_SIZE = 32
RECORD_SIZE = 4


class AssemblyError(RuntimeError):
    """A fail-closed corpus assembly or provenance error."""


@dataclass(frozen=True)
class CorpusConfig:
    repo_root: Path
    inputs: tuple[Path, ...]
    output: Path
    partition: str
    shuffle_seed: str
    minimum_games: int
    minimum_scored_positions: int
    disjoint_from: tuple[Path, ...] = ()
    quarantine_cross_partition_games: bool = False
    quarantine_duplicate_opening_games: bool = False


@dataclass(frozen=True)
class GameEntry:
    source_sha256: str
    source_game_index: int
    blob: bytes
    opening_group: str
    position_keys: tuple[bytes, ...]


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--input", action="append", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--partition", required=True, choices=PARTITIONS)
    parser.add_argument("--shuffle-seed", required=True)
    parser.add_argument("--min-games", type=int, default=1)
    parser.add_argument("--min-scored-positions", type=int, default=1)
    parser.add_argument("--disjoint-from", action="append", default=[], type=Path)
    parser.add_argument(
        "--quarantine-cross-partition-games",
        action="store_true",
        help="drop complete games containing a position present in a bound partition",
    )
    parser.add_argument(
        "--quarantine-duplicate-opening-games",
        action="store_true",
        help=(
            "retain one deterministic complete game per duplicated opening group "
            "and quarantine the others"
        ),
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    repo_root = Path(__file__).resolve().parent.parent
    try:
        config = CorpusConfig(
            repo_root=repo_root,
            inputs=tuple(
                resolve_inside(path, repo_root, "input") for path in args.input
            ),
            output=resolve_inside(args.output, repo_root, "output"),
            partition=args.partition,
            shuffle_seed=args.shuffle_seed,
            minimum_games=args.min_games,
            minimum_scored_positions=args.min_scored_positions,
            disjoint_from=tuple(
                resolve_inside(path, repo_root, "disjoint corpus")
                for path in args.disjoint_from
            ),
            quarantine_cross_partition_games=args.quarantine_cross_partition_games,
            quarantine_duplicate_opening_games=(
                args.quarantine_duplicate_opening_games
            ),
        )
        manifest = assemble_corpus(config)
        manifest_path = manifest_path_for(config.output)
        response = {
            "ok": True,
            "output": manifest["artifact"],
            "manifest": file_identity(manifest_path, repo_root),
        }
    except (AssemblyError, OSError, ValueError) as error:
        print(f"assemble-nnue-corpus: {error}", file=sys.stderr)
        return 1
    print(json.dumps(response, indent=2, sort_keys=True))
    return 0


def assemble_corpus(config: CorpusConfig) -> dict[str, Any]:
    validate_config(config)
    manifest_path = manifest_path_for(config.output)
    refuse_existing(config.output)
    refuse_existing(manifest_path)
    auditor = load_auditor()

    source_records: list[tuple[Path, dict[str, Any], dict[str, Any], str, int]] = []
    reference_policy: dict[str, Any] | None = None
    for path in config.inputs:
        manifest, manifest_identity = load_and_verify_shard(
            path, config.repo_root, config.partition, auditor
        )
        policy = source_policy(manifest)
        if reference_policy is None:
            reference_policy = policy
        elif policy != reference_policy:
            raise AssemblyError(f"source policy differs for shard: {path}")
        artifact = require_mapping(manifest, "artifact")
        source_records.append(
            (
                path,
                manifest,
                manifest_identity,
                require_sha256(artifact, "sha256", "artifact"),
                require_integer(artifact, "games", "artifact", minimum=1),
            )
        )
    assert reference_policy is not None

    source_records.sort(key=lambda item: (item[3], relative(item[0], config.repo_root)))
    games: list[GameEntry] = []
    for path, _, _, source_sha256, expected_games in source_records:
        parsed = parse_games(path.read_bytes(), source_sha256, auditor)
        if len(parsed) != expected_games:
            raise AssemblyError(f"parsed game count differs from manifest: {path}")
        games.extend(parsed)

    (
        games,
        detected_duplicate_opening_groups,
        quarantined_duplicate_opening_games,
        quarantined_duplicate_opening_scored_positions,
    ) = quarantine_duplicate_openings(
        games, enabled=config.quarantine_duplicate_opening_games
    )
    opening_groups = {game.opening_group for game in games}

    disjoint_identities: list[dict[str, Any]] = []
    bound_position_keys: set[bytes] = set()
    for path in sorted(config.disjoint_from, key=lambda item: relative(item, config.repo_root)):
        identity, position_keys, disjoint_opening_groups = load_disjoint_corpus(
            path, config.repo_root, config.partition, auditor
        )
        opening_overlap = opening_groups.intersection(disjoint_opening_groups)
        if opening_overlap:
            raise AssemblyError(
                f"cross-partition opening group leakage: {len(opening_overlap)} groups"
            )
        bound_position_keys.update(position_keys)
        disjoint_identities.append(identity)

    all_current_position_keys = {
        key for game in games for key in game.position_keys
    }
    conflicting_position_keys = all_current_position_keys.intersection(
        bound_position_keys
    )
    conflicting_games = [
        game
        for game in games
        if any(key in bound_position_keys for key in game.position_keys)
    ]
    if conflicting_games and not config.quarantine_cross_partition_games:
        raise AssemblyError(
            f"cross-partition position leakage: {len(conflicting_position_keys)} "
            f"canonical keys in {len(conflicting_games)} games"
        )
    if conflicting_games:
        conflicting_ids = {id(game) for game in conflicting_games}
        retained_games = [game for game in games if id(game) not in conflicting_ids]
    else:
        retained_games = games
    quarantined_scored_positions = sum(
        len(game.position_keys) for game in conflicting_games
    )

    ordered = sorted(
        retained_games, key=lambda game: shuffle_key(config.shuffle_seed, game)
    )
    scored_positions = sum(len(game.position_keys) for game in ordered)
    if len(ordered) < config.minimum_games:
        raise AssemblyError(
            f"corpus has {len(ordered)} games after quarantine, below minimum "
            f"{config.minimum_games}"
        )
    if scored_positions < config.minimum_scored_positions:
        raise AssemblyError(
            f"corpus has {scored_positions} scored positions after quarantine, "
            f"below minimum {config.minimum_scored_positions}"
        )

    current_position_keys = {
        key for game in ordered for key in game.position_keys
    }
    residual_overlap = current_position_keys.intersection(bound_position_keys)
    if residual_overlap:
        raise AssemblyError("internal error: quarantine left cross-partition positions")

    output_bytes = b"".join(game.blob for game in ordered)
    artifact_sha256 = hashlib.sha256(output_bytes).hexdigest()
    order_sha256 = order_digest(config.shuffle_seed, ordered)
    position_keys_sha256 = digest_keys(current_position_keys)
    config.output.parent.mkdir(parents=True, exist_ok=True)
    raw_path = config.output.with_name(f".{config.output.name}.tmp-{os.getpid()}")
    refuse_existing(raw_path)
    try:
        write_exclusive(raw_path, output_bytes)
        audit = independent_audit(raw_path, auditor)
        if audit["games"] != len(ordered):
            raise AssemblyError("output audit game count differs from assembled count")
        if audit["scored_positions"] != scored_positions:
            raise AssemblyError("output audit position count differs from assembled count")
        manifest = build_manifest(
            config,
            source_records,
            reference_policy,
            artifact_sha256,
            len(output_bytes),
            order_sha256,
            len(ordered),
            scored_positions,
            len(current_position_keys),
            position_keys_sha256,
            disjoint_identities,
            detected_duplicate_opening_groups,
            quarantined_duplicate_opening_games,
            quarantined_duplicate_opening_scored_positions,
            len(conflicting_position_keys),
            len(conflicting_games),
            quarantined_scored_positions,
            audit,
        )
        manifest_bytes = (
            json.dumps(manifest, indent=2, sort_keys=True, ensure_ascii=False) + "\n"
        ).encode("utf-8")
        publish_manifest_then_output(
            raw_path, config.output, manifest_path, manifest_bytes
        )
        return manifest
    finally:
        raw_path.unlink(missing_ok=True)


def validate_config(config: CorpusConfig) -> None:
    if not config.inputs:
        raise AssemblyError("at least one input shard is required")
    if len(set(config.inputs)) != len(config.inputs):
        raise AssemblyError("input shard paths must be unique")
    if config.partition not in PARTITIONS:
        raise AssemblyError("only train and validation corpora may be assembled")
    if not config.shuffle_seed:
        raise AssemblyError("shuffle seed must not be empty")
    if config.minimum_games < 1 or config.minimum_scored_positions < 1:
        raise AssemblyError("minimum game and position counts must be positive")
    if config.quarantine_cross_partition_games and not config.disjoint_from:
        raise AssemblyError("cross-partition quarantine requires a disjoint corpus")
    for path, label in [
        *((path, "input") for path in config.inputs),
        *((path, "disjoint corpus") for path in config.disjoint_from),
        (config.output, "output"),
    ]:
        ensure_inside(path, config.repo_root, label)
    for path in (*config.inputs, *config.disjoint_from):
        if not path.is_file():
            raise AssemblyError(f"source is not a file: {path}")
    if config.output in config.inputs or config.output in config.disjoint_from:
        raise AssemblyError("output must differ from every source")


def load_and_verify_shard(
    path: Path, repo_root: Path, partition: str, auditor
) -> tuple[dict[str, Any], dict[str, Any]]:
    manifest_path = manifest_path_for(path)
    manifest = load_json_object(manifest_path, "shard manifest")
    if manifest.get("schema") != SHARD_SCHEMA:
        raise AssemblyError(f"invalid shard manifest schema: {manifest_path}")
    if manifest.get("contract") != CONTRACT:
        raise AssemblyError(f"invalid shard codec contract: {manifest_path}")
    artifact = require_mapping(manifest, "artifact")
    expected_path = relative(path, repo_root)
    if artifact.get("path") != expected_path:
        raise AssemblyError(f"artifact path differs from shard manifest: {path}")
    actual_sha256, actual_bytes = sha256_file(path)
    if require_sha256(artifact, "sha256", "artifact") != actual_sha256:
        raise AssemblyError(f"artifact SHA-256 differs from shard manifest: {path}")
    if require_integer(artifact, "bytes", "artifact", minimum=1) != actual_bytes:
        raise AssemblyError(f"artifact byte count differs from shard manifest: {path}")
    split = require_mapping(manifest, "split")
    if split.get("partition") != partition:
        raise AssemblyError(f"shard partition differs from requested corpus: {path}")
    audit = independent_audit(path, auditor)
    compare_audit_to_artifact(audit, artifact, path)
    compare_audits(audit, require_mapping(manifest, "audit"), path)
    return manifest, file_identity(manifest_path, repo_root)


def load_disjoint_corpus(
    path: Path, repo_root: Path, partition: str, auditor
) -> tuple[dict[str, Any], set[bytes], set[str]]:
    manifest_path = manifest_path_for(path)
    manifest = load_json_object(manifest_path, "corpus manifest")
    if manifest.get("schema") != SCHEMA or manifest.get("contract") != CONTRACT:
        raise AssemblyError(f"invalid disjoint corpus manifest: {manifest_path}")
    if manifest.get("partition") == partition:
        raise AssemblyError("disjoint corpus must belong to another partition")
    artifact = require_mapping(manifest, "artifact")
    actual_sha256, actual_bytes = sha256_file(path)
    if require_sha256(artifact, "sha256", "artifact") != actual_sha256:
        raise AssemblyError(f"disjoint corpus SHA-256 differs from manifest: {path}")
    if require_integer(artifact, "bytes", "artifact", minimum=1) != actual_bytes:
        raise AssemblyError(f"disjoint corpus byte count differs from manifest: {path}")
    audit = independent_audit(path, auditor)
    compare_audit_to_artifact(audit, artifact, path)
    entries = parse_games(path.read_bytes(), actual_sha256, auditor)
    position_keys = {key for entry in entries for key in entry.position_keys}
    opening_groups = {entry.opening_group for entry in entries}
    recorded = require_mapping(manifest, "position_keys")
    if require_integer(recorded, "unique", "position_keys", minimum=1) != len(
        position_keys
    ):
        raise AssemblyError(f"disjoint corpus position-key count differs: {path}")
    if require_sha256(recorded, "sha256", "position_keys") != digest_keys(
        position_keys
    ):
        raise AssemblyError(f"disjoint corpus position-key digest differs: {path}")
    return {
        "artifact": {
            "path": relative(path, repo_root),
            "sha256": actual_sha256,
            "games": audit["games"],
            "scored_positions": audit["scored_positions"],
        },
        "manifest": file_identity(manifest_path, repo_root),
        "partition": manifest.get("partition"),
        "position_keys_sha256": digest_keys(position_keys),
    }, position_keys, opening_groups


def parse_games(data: bytes, source_sha256: str, auditor) -> list[GameEntry]:
    entries: list[GameEntry] = []
    offset = 0
    while offset < len(data):
        game_offset = offset
        if len(data) - offset < HEADER_SIZE:
            raise AssemblyError(f"truncated game header at byte {offset}")
        board, _ = auditor.decode_header(data[offset : offset + HEADER_SIZE], offset)
        opening_group = opening_group_key(board)
        offset += HEADER_SIZE
        position_keys: list[bytes] = []
        while offset < len(data):
            if len(data) - offset < RECORD_SIZE:
                raise AssemblyError(f"truncated move record at byte {offset}")
            encoded, score = struct.unpack_from("<Hh", data, offset)
            offset += RECORD_SIZE
            if encoded == 0 and score == 0:
                break
            position_keys.append(position_key(board))
            move, _ = auditor.decode_move(board, encoded, len(position_keys) - 1)
            if move not in board.legal_moves:
                raise AssemblyError(f"illegal move while indexing game at byte {game_offset}")
            board.push(move)
        else:
            raise AssemblyError(f"game at byte {game_offset} has no terminator")
        entries.append(
            GameEntry(
                source_sha256=source_sha256,
                source_game_index=len(entries),
                blob=data[game_offset:offset],
                opening_group=opening_group,
                position_keys=tuple(position_keys),
            )
        )
    return entries


def quarantine_duplicate_openings(
    games: list[GameEntry], *, enabled: bool
) -> tuple[list[GameEntry], int, int, int]:
    by_opening: dict[str, list[GameEntry]] = {}
    for game in games:
        by_opening.setdefault(game.opening_group, []).append(game)
    duplicate_groups = {
        opening: entries for opening, entries in by_opening.items() if len(entries) > 1
    }
    if duplicate_groups and not enabled:
        raise AssemblyError("an opening group appears in more than one source game")

    retained: list[GameEntry] = []
    quarantined: list[GameEntry] = []
    for opening in sorted(by_opening):
        entries = sorted(by_opening[opening], key=opening_dedup_key)
        retained.append(entries[0])
        quarantined.extend(entries[1:])
    return (
        retained,
        len(duplicate_groups),
        len(quarantined),
        sum(len(game.position_keys) for game in quarantined),
    )


def opening_group_key(board: chess.Board) -> str:
    canonical = canonical_fen(board)
    mirrored = canonical_fen(board.mirror())
    return min(canonical, mirrored)


def canonical_fen(board: chess.Board) -> str:
    return " ".join(board.fen(en_passant="fen").split()[:4])


def position_key(board: chess.Board) -> bytes:
    return hashlib.sha256(canonical_fen(board).encode("ascii")).digest()


def shuffle_key(seed: str, game: GameEntry) -> bytes:
    return stable_digest(
        SHUFFLE_SCHEMA,
        seed,
        game.source_sha256,
        str(game.source_game_index),
        hashlib.sha256(game.blob).hexdigest(),
    )


def opening_dedup_key(game: GameEntry) -> bytes:
    return stable_digest(
        OPENING_DEDUP_SCHEMA,
        game.source_sha256,
        str(game.source_game_index),
        hashlib.sha256(game.blob).hexdigest(),
    )


def order_digest(seed: str, games: list[GameEntry]) -> str:
    digest = hashlib.sha256()
    for game in games:
        digest.update(shuffle_key(seed, game))
    return digest.hexdigest()


def digest_keys(keys: set[bytes]) -> str:
    digest = hashlib.sha256()
    for key in sorted(keys):
        digest.update(key)
    return digest.hexdigest()


def source_policy(manifest: dict[str, Any]) -> dict[str, Any]:
    generator = require_mapping(manifest, "generator")
    split = require_mapping(manifest, "split")
    return {
        "contract": manifest.get("contract"),
        "generator": {
            key: generator.get(key)
            for key in (
                "binary_sha256",
                "source_commit",
                "threads",
                "hash_megabytes",
                "nodes_per_move",
                "wall_time_limit",
                "network",
                "engine_role",
            )
        },
        "split": {
            key: split.get(key)
            for key in (
                "schema",
                "seed",
                "train_percent",
                "validation_percent",
                "holdout_percent",
            )
        },
        "scoring": require_mapping(manifest, "scoring"),
        "adjudication": require_mapping(manifest, "adjudication"),
        "filter": require_mapping(manifest, "filter"),
        "source": require_mapping(manifest, "source"),
    }


def build_manifest(
    config: CorpusConfig,
    source_records: list[tuple[Path, dict[str, Any], dict[str, Any], str, int]],
    policy: dict[str, Any],
    artifact_sha256: str,
    artifact_bytes: int,
    order_sha256: str,
    games: int,
    scored_positions: int,
    unique_position_keys: int,
    position_keys_sha256: str,
    disjoint_identities: list[dict[str, Any]],
    detected_duplicate_opening_groups: int,
    quarantined_duplicate_opening_games: int,
    quarantined_duplicate_opening_scored_positions: int,
    detected_conflicting_position_keys: int,
    quarantined_games: int,
    quarantined_scored_positions: int,
    audit: dict[str, Any],
) -> dict[str, Any]:
    inputs = []
    for path, manifest, manifest_identity, _, _ in source_records:
        inputs.append(
            {
                "artifact": require_mapping(manifest, "artifact"),
                "manifest": manifest_identity,
                "opening_source": require_mapping(manifest, "opening_source"),
                "split_membership_sha256": require_sha256(
                    require_mapping(manifest, "split"), "membership_sha256", "split"
                ),
                "source_path": relative(path, config.repo_root),
            }
        )
    duplicate_positions = scored_positions - unique_position_keys
    return {
        "schema": SCHEMA,
        "contract": CONTRACT,
        "partition": config.partition,
        "artifact": {
            "path": relative(config.output, config.repo_root),
            "format": "deterministically shuffled contiguous Viriformat games",
            "bytes": artifact_bytes,
            "sha256": artifact_sha256,
            "games": games,
            "scored_positions": scored_positions,
        },
        "shuffle": {
            "schema": SHUFFLE_SCHEMA,
            "seed": config.shuffle_seed,
            "unit": "complete game blob",
            "algorithm": (
                "ascending SHA-256 framed digest of schema, seed, source artifact "
                "SHA-256, zero-based source game index, and game SHA-256"
            ),
            "order_sha256": order_sha256,
        },
        "position_keys": {
            "definition": "SHA-256 of canonical first-four-field FEN before each move",
            "unique": unique_position_keys,
            "duplicate_occurrences_within_partition": duplicate_positions,
            "sha256": position_keys_sha256,
        },
        "separation": {
            "duplicate_opening_groups": 0,
            "detected_duplicate_opening_groups": detected_duplicate_opening_groups,
            "quarantined_duplicate_opening_games": (
                quarantined_duplicate_opening_games
            ),
            "quarantined_duplicate_opening_scored_positions": (
                quarantined_duplicate_opening_scored_positions
            ),
            "opening_group_policy": (
                "retain-deterministic-winner-drop-other-complete-games"
                if config.quarantine_duplicate_opening_games
                else "reject"
            ),
            "cross_partition_position_keys": 0,
            "policy": (
                "drop-conflicting-complete-game"
                if config.quarantine_cross_partition_games
                else "reject"
            ),
            "detected_conflicting_position_keys": detected_conflicting_position_keys,
            "quarantined_games": quarantined_games,
            "quarantined_scored_positions": quarantined_scored_positions,
            "disjoint_from": disjoint_identities,
        },
        "inputs": inputs,
        "source_policy": policy,
        "source_policy_sha256": hashlib.sha256(canonical_json(policy)).hexdigest(),
        "minimums": {
            "games": config.minimum_games,
            "scored_positions": config.minimum_scored_positions,
        },
        "audit": audit,
        "environment": {
            "python": sys.version.split()[0],
            "python_chess": chess.__version__,
            "platform": platform.platform(),
            "machine": platform.machine(),
        },
    }


def independent_audit(path: Path, auditor) -> dict[str, Any]:
    try:
        return auditor.audit_file(path, require_completed_games=True)
    except auditor.AuditError as error:
        raise AssemblyError(f"independent corpus audit failed: {error}") from error


def compare_audit_to_artifact(
    audit: dict[str, Any], artifact: dict[str, Any], path: Path
) -> None:
    for audit_key, artifact_key in (
        ("sha256", "sha256"),
        ("bytes", "bytes"),
        ("games", "games"),
        ("scored_positions", "scored_positions"),
    ):
        if audit.get(audit_key) != artifact.get(artifact_key):
            raise AssemblyError(f"independent audit differs from artifact metadata: {path}")


def compare_audits(audit: dict[str, Any], recorded: dict[str, Any], path: Path) -> None:
    for key in (
        "sha256",
        "bytes",
        "games",
        "scored_positions",
        "max_game_plies",
        "results",
        "score_cp",
        "special_moves",
        "completion_reasons",
    ):
        if audit.get(key) != recorded.get(key):
            raise AssemblyError(f"independent audit differs from shard audit for {key}: {path}")


def load_auditor():
    path = Path(__file__).resolve().with_name("audit-nnue-data.py")
    spec = importlib.util.spec_from_file_location("neyrang_corpus_independent_auditor", path)
    if spec is None or spec.loader is None:
        raise AssemblyError("cannot load independent NNUE auditor")
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


def load_json_object(path: Path, label: str) -> dict[str, Any]:
    try:
        payload = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeDecodeError, json.JSONDecodeError) as error:
        raise AssemblyError(f"invalid {label}: {path}: {error}") from error
    if not isinstance(payload, dict):
        raise AssemblyError(f"{label} must contain a JSON object: {path}")
    return payload


def require_mapping(parent: dict[str, Any], key: str) -> dict[str, Any]:
    value = parent.get(key)
    if not isinstance(value, dict):
        raise AssemblyError(f"manifest field {key} must be an object")
    return value


def require_integer(
    parent: dict[str, Any], key: str, label: str, *, minimum: int
) -> int:
    value = parent.get(key)
    if not isinstance(value, int) or isinstance(value, bool) or value < minimum:
        raise AssemblyError(f"{label} field {key} is invalid")
    return value


def require_sha256(parent: dict[str, Any], key: str, label: str) -> str:
    value = parent.get(key)
    if (
        not isinstance(value, str)
        or len(value) != 64
        or any(character not in "0123456789abcdef" for character in value)
    ):
        raise AssemblyError(f"{label} field {key} is not a lowercase SHA-256")
    return value


def canonical_json(value: Any) -> bytes:
    return json.dumps(
        value, sort_keys=True, separators=(",", ":"), ensure_ascii=False
    ).encode("utf-8")


def stable_digest(*parts: str) -> bytes:
    digest = hashlib.sha256()
    for part in parts:
        encoded = part.encode("utf-8")
        digest.update(len(encoded).to_bytes(8, "big"))
        digest.update(encoded)
    return digest.digest()


def sha256_file(path: Path) -> tuple[str, int]:
    digest = hashlib.sha256()
    byte_count = 0
    with path.open("rb") as handle:
        while chunk := handle.read(1024 * 1024):
            digest.update(chunk)
            byte_count += len(chunk)
    return digest.hexdigest(), byte_count


def file_identity(path: Path, repo_root: Path) -> dict[str, Any]:
    digest, byte_count = sha256_file(path)
    return {
        "path": relative(path, repo_root),
        "bytes": byte_count,
        "sha256": digest,
    }


def manifest_path_for(output: Path) -> Path:
    return Path(f"{output}.manifest.json")


def write_exclusive(path: Path, data: bytes) -> None:
    with path.open("xb") as handle:
        handle.write(data)
        handle.flush()
        os.fsync(handle.fileno())


def publish_manifest_then_output(
    raw_output: Path, output: Path, manifest: Path, manifest_bytes: bytes
) -> None:
    manifest_temp = manifest.with_name(f".{manifest.name}.tmp-{os.getpid()}")
    refuse_existing(manifest_temp)
    manifest_committed = False
    output_committed = False
    try:
        write_exclusive(manifest_temp, manifest_bytes)
        link_without_overwrite(manifest_temp, manifest)
        manifest_committed = True
        link_without_overwrite(raw_output, output)
        output_committed = True
    except BaseException:
        manifest_temp.unlink(missing_ok=True)
        if manifest_committed and not output_committed:
            manifest.unlink(missing_ok=True)
        if output_committed and not manifest_committed:
            output.unlink(missing_ok=True)
        raise


def link_without_overwrite(temporary: Path, destination: Path) -> None:
    try:
        os.link(temporary, destination)
    except FileExistsError as error:
        raise AssemblyError(f"refusing to overwrite existing path: {destination}") from error
    temporary.unlink()


def refuse_existing(path: Path) -> None:
    if path.exists():
        raise AssemblyError(f"refusing to overwrite existing path: {path}")


def resolve_inside(path: Path, repo_root: Path, label: str) -> Path:
    candidate = path if path.is_absolute() else repo_root / path
    resolved = candidate.resolve()
    ensure_inside(resolved, repo_root, label)
    return resolved


def ensure_inside(path: Path, repo_root: Path, label: str) -> None:
    try:
        path.resolve().relative_to(repo_root.resolve())
    except ValueError as error:
        raise AssemblyError(f"{label} must be inside the repository") from error


def relative(path: Path, repo_root: Path) -> str:
    return path.resolve().relative_to(repo_root.resolve()).as_posix()


if __name__ == "__main__":
    raise SystemExit(main())
