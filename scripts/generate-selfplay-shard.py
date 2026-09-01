#!/usr/bin/env python3
"""Generate one partitioned, independently audited NEYRANG self-play shard."""

from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import os
import platform
import re
import selectors
import subprocess
import sys
import time
from collections import Counter
from dataclasses import dataclass
from pathlib import Path
from typing import Any

import chess


SCHEMA = "neyrang-nnue-selfplay-shard-v1"
SPLIT_SCHEMA = "neyrang-nnue-selfplay-split-v1"
LEGACY_SPLIT_SCHEMA = "neyrang-nnue-selfplay-split-v1"
SPLIT_DIGEST_SCHEMAS = (SPLIT_SCHEMA, LEGACY_SPLIT_SCHEMA)
PARTITIONS = ("train", "validation", "holdout")
OPENING_MANIFEST_SCHEMAS = (
    "neyrang-genfens-shard-v1",
    "neyrang-genfens-shard-v1",
)
MAX_CAPTURED_STREAM_BYTES = 1 << 20
PROGRESS = re.compile(
    r"^info string selfplay attempted ([0-9]+) accepted ([0-9]+) "
    r"rejected ([0-9]+) positions ([0-9]+)$"
)
SUMMARY_COUNT_FIELDS = (
    "attempted_games",
    "accepted_games",
    "rejected_games",
    "accepted_positions",
    "checkmates",
    "stalemates",
    "fifty_move_draws",
    "threefold_draws",
    "rejected_terminal_openings",
    "rejected_maximum_plies",
    "rejected_missing_search_move",
    "rejected_illegal_search_move",
    "rejected_search_mutated_position",
)


class GenerationError(RuntimeError):
    """A fail-closed self-play generation or provenance error."""


@dataclass(frozen=True)
class ShardConfig:
    repo_root: Path
    generator: Path
    openings: Path
    openings_manifest: Path
    output: Path
    partition: str
    split_seed: str
    train_percent: int
    validation_percent: int
    nodes_per_move: int
    hash_megabytes: int
    maximum_plies: int
    generator_source_commit: str
    compiler_identity: str
    source_license: str
    stall_timeout_seconds: float = 60.0
    split_digest_schema: str = SPLIT_SCHEMA


@dataclass(frozen=True)
class OpeningAssignment:
    index: int
    fen: str
    group_key: str
    partition: str


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--generator", required=True, type=Path)
    parser.add_argument("--openings", required=True, type=Path)
    parser.add_argument("--openings-manifest", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--partition", required=True, choices=PARTITIONS)
    parser.add_argument("--split-seed", required=True)
    parser.add_argument("--train-percent", type=int, default=80)
    parser.add_argument("--validation-percent", type=int, default=10)
    parser.add_argument("--nodes", required=True, type=int)
    parser.add_argument("--hash-mb", required=True, type=int)
    parser.add_argument("--max-plies", required=True, type=int)
    parser.add_argument("--generator-source-commit", required=True)
    parser.add_argument("--compiler-identity", required=True)
    parser.add_argument("--source-license", required=True)
    parser.add_argument("--stall-timeout-seconds", type=float, default=60.0)
    parser.add_argument(
        "--split-digest-schema",
        choices=SPLIT_DIGEST_SCHEMAS,
        default=SPLIT_SCHEMA,
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    repo_root = Path(__file__).resolve().parent.parent
    try:
        config = ShardConfig(
            repo_root=repo_root,
            generator=resolve_inside(args.generator, repo_root, "generator"),
            openings=resolve_inside(args.openings, repo_root, "openings"),
            openings_manifest=resolve_inside(
                args.openings_manifest, repo_root, "openings manifest"
            ),
            output=resolve_inside(args.output, repo_root, "output"),
            partition=args.partition,
            split_seed=args.split_seed,
            train_percent=args.train_percent,
            validation_percent=args.validation_percent,
            nodes_per_move=args.nodes,
            hash_megabytes=args.hash_mb,
            maximum_plies=args.max_plies,
            generator_source_commit=args.generator_source_commit,
            compiler_identity=args.compiler_identity,
            source_license=args.source_license,
            stall_timeout_seconds=args.stall_timeout_seconds,
            split_digest_schema=args.split_digest_schema,
        )
        manifest = generate_shard(config)
        manifest_path = manifest_path_for(config.output)
        manifest_sha256, manifest_bytes = sha256_file(manifest_path)
        response = {
            "ok": True,
            "output": manifest["artifact"],
            "manifest": {
                "path": relative(manifest_path, repo_root),
                "bytes": manifest_bytes,
                "sha256": manifest_sha256,
            },
        }
    except (GenerationError, OSError, ValueError) as error:
        print(f"generate-selfplay-shard: {error}", file=sys.stderr)
        return 1
    print(json.dumps(response, indent=2, sort_keys=True))
    return 0


def generate_shard(config: ShardConfig) -> dict[str, Any]:
    validate_config(config)
    manifest_path = manifest_path_for(config.output)
    refuse_existing(config.output)
    refuse_existing(manifest_path)
    opening_manifest, opening_manifest_identity = verify_opening_manifest(config)
    fens = read_and_audit_openings(config.openings)
    if opening_manifest["artifact"].get("openings") != len(fens):
        raise GenerationError("opening count differs from its manifest")
    assignments = assign_partitions(fens, config)
    selected = [item for item in assignments if item.partition == config.partition]
    if not selected:
        raise GenerationError(
            f"opening shard has no groups assigned to partition {config.partition}"
        )

    config.output.parent.mkdir(parents=True, exist_ok=True)
    process_id = os.getpid()
    selected_path = config.output.with_name(f".{config.output.name}.selected-{process_id}.epd")
    raw_path = config.output.with_name(f".{config.output.name}.raw-{process_id}.vf")
    summary_path = config.output.with_name(f".{config.output.name}.summary-{process_id}.json")
    for temporary in (selected_path, raw_path, summary_path):
        refuse_existing(temporary)
    selected_bytes = "".join(f"{item.fen}\n" for item in selected).encode("utf-8")
    write_exclusive(selected_path, selected_bytes)

    command = [
        str(config.generator),
        "--openings",
        str(selected_path),
        "--output",
        str(raw_path),
        "--summary",
        str(summary_path),
        "--nodes",
        str(config.nodes_per_move),
        "--hash-mb",
        str(config.hash_megabytes),
        "--max-plies",
        str(config.maximum_plies),
    ]
    try:
        final_progress = run_generator(command, len(selected), config.stall_timeout_seconds)
        summary = read_and_validate_summary(summary_path, len(selected), final_progress)
        allowed_initial_positions = {
            chess.Board(item.fen).fen(en_passant="fen") for item in selected
        }
        audit = independent_audit(raw_path, allowed_initial_positions)
        if audit["games"] != summary["accepted_games"]:
            raise GenerationError("independent game count differs from generator summary")
        if audit["scored_positions"] != summary["accepted_positions"]:
            raise GenerationError("independent position count differs from generator summary")
        expected_completion_reasons = {
            reason: summary[field]
            for reason, field in (
                ("checkmate", "checkmates"),
                ("stalemate", "stalemates"),
                ("fifty_move_rule", "fifty_move_draws"),
                ("threefold_repetition", "threefold_draws"),
            )
            if summary[field]
        }
        if audit["completion_reasons"] != expected_completion_reasons:
            raise GenerationError(
                "generator completion reasons differ from independent replay"
            )

        artifact_sha256, artifact_bytes = sha256_file(raw_path)
        generator_sha256, generator_bytes = sha256_file(config.generator)
        selected_sha256 = hashlib.sha256(selected_bytes).hexdigest()
        manifest = build_manifest(
            config,
            opening_manifest,
            opening_manifest_identity,
            assignments,
            selected,
            selected_sha256,
            artifact_sha256,
            artifact_bytes,
            generator_sha256,
            generator_bytes,
            command,
            summary,
            audit,
        )
        manifest_bytes = (
            json.dumps(manifest, indent=2, sort_keys=True, ensure_ascii=False) + "\n"
        ).encode("utf-8")
        publish_manifest_then_output(raw_path, config.output, manifest_path, manifest_bytes)
        return manifest
    finally:
        selected_path.unlink(missing_ok=True)
        raw_path.unlink(missing_ok=True)
        summary_path.unlink(missing_ok=True)


def validate_config(config: ShardConfig) -> None:
    root = config.repo_root.resolve()
    for path, label in [
        (config.generator, "generator"),
        (config.openings, "openings"),
        (config.openings_manifest, "openings manifest"),
        (config.output, "output"),
    ]:
        try:
            path.resolve().relative_to(root)
        except ValueError as error:
            raise GenerationError(f"{label} must be inside the repository") from error
    if not config.generator.is_file() or not os.access(config.generator, os.X_OK):
        raise GenerationError(f"generator must be an executable file: {config.generator}")
    if not config.openings.is_file():
        raise GenerationError(f"openings are not a file: {config.openings}")
    if not config.openings_manifest.is_file():
        raise GenerationError(
            f"openings manifest is not a file: {config.openings_manifest}"
        )
    if config.partition not in PARTITIONS:
        raise GenerationError(f"partition must be one of {', '.join(PARTITIONS)}")
    if not config.split_seed:
        raise GenerationError("split seed must not be empty")
    if config.split_digest_schema not in SPLIT_DIGEST_SCHEMAS:
        raise GenerationError("split digest schema is not registered")
    if config.train_percent <= 0 or config.validation_percent <= 0:
        raise GenerationError("split percentages must be positive")
    if config.train_percent + config.validation_percent >= 100:
        raise GenerationError("train plus validation percentage must be below 100")
    if config.nodes_per_move <= 0 or config.hash_megabytes <= 0:
        raise GenerationError("nodes and hash-mb must be positive")
    if config.hash_megabytes > 65_536:
        raise GenerationError("hash-mb must not exceed 65536")
    if not 1 <= config.maximum_plies <= 1_024:
        raise GenerationError("max-plies must be between 1 and 1024")
    if not re.fullmatch(r"[0-9a-fA-F]{7,64}", config.generator_source_commit):
        raise GenerationError("generator source commit must be a 7..64 digit hex identity")
    if not config.compiler_identity.strip() or not config.source_license.strip():
        raise GenerationError("compiler identity and source license must not be empty")
    if not 0 < config.stall_timeout_seconds <= 3_600:
        raise GenerationError("stall timeout must be within (0, 3600] seconds")


def verify_opening_manifest(
    config: ShardConfig,
) -> tuple[dict[str, Any], dict[str, Any]]:
    try:
        payload = json.loads(config.openings_manifest.read_text(encoding="utf-8"))
    except (json.JSONDecodeError, UnicodeDecodeError) as error:
        raise GenerationError(f"invalid openings manifest JSON: {error}") from error
    if not isinstance(payload, dict) or payload.get("schema") not in OPENING_MANIFEST_SCHEMAS:
        raise GenerationError(
            "openings manifest schema must be neyrang-genfens-shard-v1 "
            "or the registered legacy neyrang-genfens-shard-v1"
        )
    artifact = require_mapping(payload, "artifact")
    artifact_path = artifact.get("path")
    if not isinstance(artifact_path, str):
        raise GenerationError("openings manifest artifact path must be a string")
    declared_path = resolve_inside(Path(artifact_path), config.repo_root, "manifest artifact")
    if declared_path != config.openings.resolve():
        raise GenerationError("openings manifest points to a different artifact")
    digest, byte_count = sha256_file(config.openings)
    if artifact.get("sha256") != digest:
        raise GenerationError("opening artifact SHA-256 differs from its manifest")
    if artifact.get("bytes") != byte_count:
        raise GenerationError("opening artifact byte count differs from its manifest")
    source = require_mapping(payload, "source")
    if source.get("license") != config.source_license:
        raise GenerationError("opening source license differs from requested source license")
    manifest_sha256, manifest_bytes = sha256_file(config.openings_manifest)
    return payload, {
        "path": relative(config.openings_manifest, config.repo_root),
        "bytes": manifest_bytes,
        "sha256": manifest_sha256,
    }


def read_and_audit_openings(path: Path) -> list[str]:
    data = path.read_bytes()
    if not data or not data.endswith(b"\n") or b"\r" in data:
        raise GenerationError("opening shard must be non-empty and LF-terminated")
    try:
        text = data.decode("utf-8")
    except UnicodeDecodeError as error:
        raise GenerationError(f"opening shard is not UTF-8: {error}") from error
    fens: list[str] = []
    canonical_positions: set[str] = set()
    for index, fen in enumerate(text.splitlines(), start=1):
        if len(fen.split()) != 6:
            raise GenerationError(f"invalid FEN at opening {index}: expected six fields")
        try:
            board = chess.Board(fen)
        except ValueError as error:
            raise GenerationError(f"invalid FEN at opening {index}: {error}") from error
        if not board.is_valid() or board.is_check() or board.is_game_over(claim_draw=False):
            raise GenerationError(f"opening {index} is invalid, checked, or terminal")
        canonical = " ".join(board.fen(en_passant="fen").split()[:4])
        if canonical in canonical_positions:
            raise GenerationError(f"duplicate canonical position at opening {index}")
        canonical_positions.add(canonical)
        fens.append(board.fen(en_passant="fen"))
    return fens


def opening_group_key(fen: str) -> str:
    board = chess.Board(fen)
    canonical = " ".join(board.fen(en_passant="fen").split()[:4])
    mirrored = " ".join(board.mirror().fen(en_passant="fen").split()[:4])
    return min(canonical, mirrored)


def partition_for_opening(
    fen: str,
    split_seed: str,
    train_percent: int,
    validation_percent: int,
    *,
    split_digest_schema: str = SPLIT_SCHEMA,
) -> str:
    if split_digest_schema not in SPLIT_DIGEST_SCHEMAS:
        raise GenerationError("split digest schema is not registered")
    digest = stable_digest(
        split_digest_schema, split_seed, opening_group_key(fen), "partition"
    )
    bucket = int.from_bytes(digest[:8], "big") % 100
    if bucket < train_percent:
        return "train"
    if bucket < train_percent + validation_percent:
        return "validation"
    return "holdout"


def assign_partitions(fens: list[str], config: ShardConfig) -> list[OpeningAssignment]:
    return [
        OpeningAssignment(
            index=index,
            fen=fen,
            group_key=opening_group_key(fen),
            partition=partition_for_opening(
                fen,
                config.split_seed,
                config.train_percent,
                config.validation_percent,
                split_digest_schema=config.split_digest_schema,
            ),
        )
        for index, fen in enumerate(fens)
    ]


def run_generator(
    command: list[str], expected_attempts: int, stall_timeout_seconds: float
) -> tuple[int, int, int, int]:
    process = subprocess.Popen(
        command,
        stdin=subprocess.DEVNULL,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        bufsize=0,
    )
    assert process.stdout is not None and process.stderr is not None
    selector = selectors.DefaultSelector()
    selector.register(process.stdout, selectors.EVENT_READ, "stdout")
    selector.register(process.stderr, selectors.EVENT_READ, "stderr")
    buffers = {"stdout": bytearray(), "stderr": bytearray()}
    deadline = time.monotonic() + stall_timeout_seconds
    last_progress: tuple[int, int, int, int] | None = None
    try:
        while selector.get_map():
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                raise GenerationError(
                    f"generator stalled for {stall_timeout_seconds:g} seconds"
                )
            events = selector.select(remaining)
            if not events:
                if process.poll() is not None:
                    for key in list(selector.get_map().values()):
                        name = key.data
                        chunk = os.read(key.fileobj.fileno(), 65_536)
                        if chunk:
                            buffers[name].extend(chunk)
                            if name == "stdout":
                                last_progress, _ = consume_progress(
                                    buffers[name], last_progress, expected_attempts
                                )
                        selector.unregister(key.fileobj)
                    continue
                raise GenerationError(
                    f"generator stalled for {stall_timeout_seconds:g} seconds"
                )
            for key, _ in events:
                name = key.data
                chunk = os.read(key.fileobj.fileno(), 65_536)
                if not chunk:
                    selector.unregister(key.fileobj)
                    continue
                buffers[name].extend(chunk)
                if len(buffers[name]) > MAX_CAPTURED_STREAM_BYTES:
                    raise GenerationError(f"generator {name} exceeded 1 MiB")
                if name == "stdout":
                    last_progress, advanced = consume_progress(
                        buffers[name], last_progress, expected_attempts
                    )
                    if advanced:
                        deadline = time.monotonic() + stall_timeout_seconds
        if buffers["stdout"]:
            raise GenerationError("generator emitted a non-LF-terminated stdout line")
        return_code = process.wait(timeout=1.0)
        stderr = decode_stream(buffers["stderr"], "stderr")
        if return_code != 0:
            raise GenerationError(
                f"generator exited with status {return_code}: {stderr.strip()}"
            )
        if stderr:
            raise GenerationError(f"generator emitted stderr: {stderr.strip()}")
        if last_progress is None or last_progress[0] != expected_attempts:
            raise GenerationError(
                f"generator progress did not reach {expected_attempts} attempts"
            )
        return last_progress
    except BaseException:
        terminate_process(process)
        raise
    finally:
        selector.close()
        process.stdout.close()
        process.stderr.close()


def consume_progress(
    buffer: bytearray,
    previous: tuple[int, int, int, int] | None,
    expected_attempts: int,
) -> tuple[tuple[int, int, int, int] | None, bool]:
    latest = previous
    advanced = False
    while True:
        newline = buffer.find(b"\n")
        if newline < 0:
            break
        raw = bytes(buffer[:newline])
        del buffer[: newline + 1]
        line = decode_stream(raw, "stdout line")
        match = PROGRESS.fullmatch(line)
        if match is None:
            raise GenerationError(f"unexpected generator stdout line: {line!r}")
        values = tuple(int(value) for value in match.groups())
        attempted, accepted, rejected, positions = values
        if accepted + rejected != attempted or attempted > expected_attempts:
            raise GenerationError(f"invalid generator progress counts: {line}")
        if latest is not None and (attempted <= latest[0] or positions < latest[3]):
            raise GenerationError(f"non-monotonic generator progress: {line}")
        latest = values
        advanced = True
    return latest, advanced


def read_and_validate_summary(
    path: Path,
    expected_attempts: int,
    progress: tuple[int, int, int, int],
) -> dict[str, int | str]:
    try:
        payload = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError, UnicodeDecodeError) as error:
        raise GenerationError(f"invalid generator summary: {error}") from error
    if not isinstance(payload, dict) or payload.get("schema") != "neyrang-selfplay-summary-v1":
        raise GenerationError("generator summary has an invalid schema")
    for field in SUMMARY_COUNT_FIELDS:
        value = payload.get(field)
        if not isinstance(value, int) or isinstance(value, bool) or value < 0:
            raise GenerationError(f"generator summary field {field} is invalid")
    if payload["attempted_games"] != expected_attempts:
        raise GenerationError("generator summary attempted count differs from selection")
    if payload["accepted_games"] + payload["rejected_games"] != expected_attempts:
        raise GenerationError("generator summary accepted/rejected counts do not balance")
    completion_total = sum(
        payload[field]
        for field in ("checkmates", "stalemates", "fifty_move_draws", "threefold_draws")
    )
    rejection_total = sum(
        payload[field]
        for field in (
            "rejected_terminal_openings",
            "rejected_maximum_plies",
            "rejected_missing_search_move",
            "rejected_illegal_search_move",
            "rejected_search_mutated_position",
        )
    )
    if completion_total != payload["accepted_games"]:
        raise GenerationError("generator completion reasons do not sum to accepted games")
    if rejection_total != payload["rejected_games"]:
        raise GenerationError("generator rejection reasons do not sum to rejected games")
    if progress != (
        payload["attempted_games"],
        payload["accepted_games"],
        payload["rejected_games"],
        payload["accepted_positions"],
    ):
        raise GenerationError("generator summary differs from final progress line")
    return payload


def independent_audit(path: Path, allowed_initial_positions: set[str]) -> dict[str, Any]:
    auditor_path = Path(__file__).resolve().with_name("audit-nnue-data.py")
    spec = importlib.util.spec_from_file_location("neyrang_independent_nnue_auditor", auditor_path)
    if spec is None or spec.loader is None:
        raise GenerationError("cannot load independent NNUE auditor")
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    try:
        return module.audit_file(
            path,
            require_completed_games=True,
            allowed_initial_positions=allowed_initial_positions,
        )
    except module.AuditError as error:
        raise GenerationError(f"independent corpus audit failed: {error}") from error


def build_manifest(
    config: ShardConfig,
    opening_manifest: dict[str, Any],
    opening_manifest_identity: dict[str, Any],
    assignments: list[OpeningAssignment],
    selected: list[OpeningAssignment],
    selected_sha256: str,
    artifact_sha256: str,
    artifact_bytes: int,
    generator_sha256: str,
    generator_bytes: int,
    command: list[str],
    summary: dict[str, Any],
    audit: dict[str, Any],
) -> dict[str, Any]:
    opening_counts = Counter(item.partition for item in assignments)
    groups = {
        partition: {item.group_key for item in assignments if item.partition == partition}
        for partition in PARTITIONS
    }
    membership_rows = "".join(
        f"{item.index}\t{hashlib.sha256(item.group_key.encode()).hexdigest()}\t{item.partition}\n"
        for item in assignments
    ).encode("ascii")
    return {
        "schema": SCHEMA,
        "contract": "neyrang-viriformat-strict-v1",
        "artifact": {
            "path": relative(config.output, config.repo_root),
            "format": "contiguous replay-validated Viriformat games",
            "bytes": artifact_bytes,
            "sha256": artifact_sha256,
            "games": summary["accepted_games"],
            "scored_positions": summary["accepted_positions"],
        },
        "generator": {
            "binary_path": relative(config.generator, config.repo_root),
            "binary_bytes": generator_bytes,
            "binary_sha256": generator_sha256,
            "source_commit": config.generator_source_commit.lower(),
            "compiler_identity": config.compiler_identity,
            "command": [relative_or_value(value, config.repo_root) for value in command],
            "threads": 1,
            "hash_megabytes": config.hash_megabytes,
            "nodes_per_move": config.nodes_per_move,
            "wall_time_limit": None,
            "network": None,
            "engine_role": "NEYRANG self-play generator and score teacher",
        },
        "opening_source": {
            "artifact": opening_manifest["artifact"],
            "manifest": opening_manifest_identity,
            "license": config.source_license,
            "selected_openings": len(selected),
            "selected_epd_sha256": selected_sha256,
        },
        "split": {
            "schema": SPLIT_SCHEMA,
            "assignment_digest_schema": config.split_digest_schema,
            "seed": config.split_seed,
            "group_key": "lexicographic minimum of canonical first-four-field FEN and color-reversed mirror",
            "train_percent": config.train_percent,
            "validation_percent": config.validation_percent,
            "holdout_percent": 100 - config.train_percent - config.validation_percent,
            "partition": config.partition,
            "openings_by_partition": {
                name: opening_counts[name] for name in PARTITIONS
            },
            "groups_by_partition": {name: len(groups[name]) for name in PARTITIONS},
            "membership_sha256": hashlib.sha256(membership_rows).hexdigest(),
            "assignment_order": "before self-play, filtering, sampling, shuffling, or interleaving",
        },
        "scoring": {
            "point_of_view": "White",
            "parent_position": True,
            "header_score": 0,
            "mate_policy": "preserve NEYRANG search centipawn/mate score before saturation",
            "saturation": "clamp signed score to i16 after Black-to-move sign inversion",
        },
        "adjudication": {
            "policy": "rules-only-v1",
            "accepted": [
                "checkmate",
                "stalemate",
                "halfmove clock >= 100",
                "third occurrence within recorded trajectory",
            ],
            "maximum_plies": config.maximum_plies,
            "maximum_plies_policy": "reject game; never synthesize a draw",
            "tablebases": None,
            "evaluation_adjudication": None,
        },
        "filter": {
            "version": "rules-complete-v1",
            "accepted_game": "legal replay ending in one registered rules-only condition",
            "rejected_game": "terminal input, search contract failure, or maximum plies",
            "position_sampling": None,
            "score_filter": None,
        },
        "counts": {field: summary[field] for field in SUMMARY_COUNT_FIELDS},
        "audit": audit,
        "environment": {
            "python": sys.version.split()[0],
            "python_chess": chess.__version__,
            "platform": platform.platform(),
            "machine": platform.machine(),
        },
        "source": {
            "kind": "NEYRANG deterministic self-play",
            "license": config.source_license,
            "external_mixing_ratio": 0,
        },
    }


def publish_manifest_then_output(
    raw_output: Path, output: Path, manifest: Path, manifest_bytes: bytes
) -> None:
    manifest_temp = manifest.with_name(f".{manifest.name}.tmp-{os.getpid()}")
    refuse_existing(manifest_temp)
    manifest_committed = False
    output_committed = False
    try:
        with raw_output.open("rb") as handle:
            os.fsync(handle.fileno())
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


def write_exclusive(path: Path, data: bytes) -> None:
    with path.open("xb") as handle:
        handle.write(data)
        handle.flush()
        os.fsync(handle.fileno())


def link_without_overwrite(temporary: Path, destination: Path) -> None:
    try:
        os.link(temporary, destination)
    except FileExistsError as error:
        raise GenerationError(
            f"refusing to overwrite existing path: {destination}"
        ) from error
    temporary.unlink()


def terminate_process(process: subprocess.Popen[bytes]) -> None:
    if process.poll() is not None:
        return
    process.terminate()
    try:
        process.wait(timeout=1.0)
    except subprocess.TimeoutExpired:
        process.kill()
        process.wait(timeout=1.0)


def require_mapping(parent: dict[str, Any], key: str) -> dict[str, Any]:
    value = parent.get(key)
    if not isinstance(value, dict):
        raise GenerationError(f"openings manifest field {key} must be an object")
    return value


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


def manifest_path_for(output: Path) -> Path:
    return Path(f"{output}.manifest.json")


def refuse_existing(path: Path) -> None:
    if path.exists():
        raise GenerationError(f"refusing to overwrite existing path: {path}")


def resolve_inside(path: Path, repo_root: Path, label: str) -> Path:
    candidate = path if path.is_absolute() else repo_root / path
    resolved = candidate.resolve()
    try:
        resolved.relative_to(repo_root.resolve())
    except ValueError as error:
        raise GenerationError(f"{label} must be inside the repository") from error
    return resolved


def relative(path: Path, repo_root: Path) -> str:
    try:
        return path.resolve().relative_to(repo_root.resolve()).as_posix()
    except ValueError as error:
        raise GenerationError(f"path is outside repository: {path}") from error


def relative_or_value(value: str, repo_root: Path) -> str:
    path = Path(value)
    if path.is_absolute():
        try:
            return path.resolve().relative_to(repo_root.resolve()).as_posix()
        except ValueError:
            return value
    return value


def decode_stream(data: bytes | bytearray, label: str) -> str:
    try:
        return bytes(data).decode("utf-8")
    except UnicodeDecodeError as error:
        raise GenerationError(f"generator {label} is not valid UTF-8") from error


if __name__ == "__main__":
    raise SystemExit(main())
