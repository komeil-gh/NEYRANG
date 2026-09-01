#!/usr/bin/env python3
"""Generate one immutable, independently audited NEYRANG genfens shard."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
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


OPENBENCH_PREFIX = "info string genfens "
MAX_OPENINGS = 1_000_000
U64_MAX = (1 << 64) - 1
MAX_CAPTURED_STREAM_BYTES = 1 << 20


class GenerationError(RuntimeError):
    """A fail-closed opening generation or provenance error."""


@dataclass(frozen=True)
class ShardConfig:
    repo_root: Path
    engine: Path
    output: Path
    count: int
    seed: int
    generator_source_commit: str
    compiler_identity: str
    source_license: str
    stall_timeout_seconds: float = 15.0
    quarantine_duplicate_openings: bool = False


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--engine", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--count", required=True, type=int)
    parser.add_argument("--seed", required=True, type=parse_u64)
    parser.add_argument("--generator-source-commit", required=True)
    parser.add_argument("--compiler-identity", required=True)
    parser.add_argument("--source-license", required=True)
    parser.add_argument("--quarantine-duplicate-openings", action="store_true")
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    repo_root = Path(__file__).resolve().parent.parent
    try:
        config = ShardConfig(
            repo_root=repo_root,
            engine=resolve_inside(args.engine, repo_root, "engine"),
            output=resolve_inside(args.output, repo_root, "output"),
            count=args.count,
            seed=args.seed,
            generator_source_commit=args.generator_source_commit,
            compiler_identity=args.compiler_identity,
            source_license=args.source_license,
            quarantine_duplicate_openings=args.quarantine_duplicate_openings,
        )
        manifest = generate_shard(config)
        manifest_path = manifest_path_for(config.output)
        manifest_sha256, manifest_bytes = sha256_file(manifest_path)
        summary = {
            "ok": True,
            "output": manifest["artifact"],
            "manifest": {
                "path": relative(manifest_path, repo_root),
                "bytes": manifest_bytes,
                "sha256": manifest_sha256,
            },
        }
    except (GenerationError, OSError, ValueError) as error:
        print(f"generate-opening-shard: {error}", file=sys.stderr)
        return 1
    print(json.dumps(summary, indent=2, sort_keys=True))
    return 0


def generate_shard(config: ShardConfig) -> dict[str, Any]:
    validate_config(config)
    manifest_path = manifest_path_for(config.output)
    refuse_existing(config.output)
    refuse_existing(manifest_path)

    command_line = f"genfens {config.count} seed {config.seed} book None"
    fens = collect_engine_output(config, command_line)
    retained_fens, audit, duplicate_indices = audit_fens(
        fens,
        quarantine_duplicates=config.quarantine_duplicate_openings,
    )
    output_bytes = ("\n".join(retained_fens) + "\n").encode("utf-8")
    output_sha256 = hashlib.sha256(output_bytes).hexdigest()
    engine_sha256, engine_bytes = sha256_file(config.engine)
    duplicate_indices_bytes = "".join(
        f"{index}\n" for index in duplicate_indices
    ).encode("ascii")

    manifest: dict[str, Any] = {
        "schema": "neyrang-genfens-shard-v1",
        "contract": "openbench-genfens-v1",
        "artifact": {
            "path": relative(config.output, config.repo_root),
            "format": "full-six-field FEN, one opening per LF-terminated line",
            "bytes": len(output_bytes),
            "sha256": output_sha256,
            "openings": len(retained_fens),
        },
        "generator": {
            "engine_path": relative(config.engine, config.repo_root),
            "engine_bytes": engine_bytes,
            "engine_sha256": engine_sha256,
            "source_commit": config.generator_source_commit.lower(),
            "compiler_identity": config.compiler_identity,
            "evaluation": "NEYRANG HCE two-ply reply filter",
            "algorithm": "neyrang-genfens-2ply-hce-v1",
            "command": [command_line, "quit"],
            "attempted_openings": config.count,
            "seed": config.seed,
            "last_seed": (config.seed + config.count - 1) & U64_MAX,
            "seed_bits": 64,
            "seed_schedule": "unsigned-64-bit wrapping seed + opening index",
            "book": None,
        },
        "source": {
            "kind": "NEYRANG deterministic self-generated openings",
            "license": config.source_license,
        },
        "deduplication": {
            "policy": (
                "retain-first-canonical-occurrence-in-seed-order"
                if config.quarantine_duplicate_openings
                else "reject"
            ),
            "attempted_openings": config.count,
            "retained_openings": len(retained_fens),
            "quarantined_duplicate_openings": len(duplicate_indices),
            "quarantined_opening_indices": duplicate_indices,
            "quarantined_opening_indices_encoding": (
                "one-based ASCII decimal, one LF-terminated index per line, "
                "occurrence order"
            ),
            "quarantined_opening_indices_sha256": hashlib.sha256(
                duplicate_indices_bytes
            ).hexdigest(),
        },
        "audit": audit,
    }
    manifest_bytes = (
        json.dumps(manifest, indent=2, sort_keys=True, ensure_ascii=False) + "\n"
    ).encode("utf-8")
    publish_manifest_then_output(config.output, output_bytes, manifest_path, manifest_bytes)
    return manifest


def validate_config(config: ShardConfig) -> None:
    root = config.repo_root.resolve()
    engine = config.engine.resolve()
    output = config.output.resolve()
    for path, label in [(engine, "engine"), (output, "output")]:
        try:
            path.relative_to(root)
        except ValueError as error:
            raise GenerationError(f"{label} must be inside the repository") from error
    if not engine.is_file():
        raise GenerationError(f"engine is not a file: {config.engine}")
    if not os.access(engine, os.X_OK):
        raise GenerationError(f"engine is not executable: {config.engine}")
    if not 1 <= config.count <= MAX_OPENINGS:
        raise GenerationError(f"count must be between 1 and {MAX_OPENINGS}")
    if not 0 <= config.seed <= U64_MAX:
        raise GenerationError("seed must be an unsigned 64-bit integer")
    if not re.fullmatch(r"[0-9a-fA-F]{7,64}", config.generator_source_commit):
        raise GenerationError("generator source commit must be a 7..64 digit hex identity")
    if not config.compiler_identity.strip():
        raise GenerationError("compiler identity must not be empty")
    if not config.source_license.strip():
        raise GenerationError("source license must not be empty")
    if not 0 < config.stall_timeout_seconds <= 15.0:
        raise GenerationError("stall timeout must be within (0, 15] seconds")


def collect_engine_output(config: ShardConfig, command_line: str) -> list[str]:
    process = subprocess.Popen(
        [str(config.engine), command_line, "quit"],
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
    fens: list[str] = []
    deadline = time.monotonic() + config.stall_timeout_seconds

    try:
        while selector.get_map():
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                raise GenerationError(
                    f"engine stalled for {config.stall_timeout_seconds:g} seconds"
                )
            events = selector.select(remaining)
            if not events:
                raise GenerationError(
                    f"engine stalled for {config.stall_timeout_seconds:g} seconds"
                )
            for key, _ in events:
                name = key.data
                chunk = os.read(key.fileobj.fileno(), 65_536)
                if not chunk:
                    selector.unregister(key.fileobj)
                    continue
                buffers[name].extend(chunk)
                if len(buffers[name]) > MAX_CAPTURED_STREAM_BYTES:
                    raise GenerationError(f"engine {name} exceeded the 1 MiB safety limit")
                if name == "stdout":
                    deadline = consume_stdout_lines(
                        buffers[name],
                        fens,
                        config.count,
                        config.stall_timeout_seconds,
                        deadline,
                    )

        if buffers["stdout"]:
            raise GenerationError("engine emitted a non-LF-terminated stdout line")
        return_code = process.wait(timeout=1.0)
        stderr_text = decode_stream(buffers["stderr"], "stderr")
        if return_code != 0:
            raise GenerationError(
                f"engine exited with status {return_code}: {stderr_text.strip()}"
            )
        if stderr_text:
            raise GenerationError(f"engine emitted stderr: {stderr_text.strip()}")
        if len(fens) != config.count:
            raise GenerationError(
                f"engine emitted {len(fens)} openings, expected {config.count}"
            )
        return fens
    except BaseException:
        terminate_process(process)
        raise
    finally:
        selector.close()
        process.stdout.close()
        process.stderr.close()


def consume_stdout_lines(
    buffer: bytearray,
    fens: list[str],
    expected: int,
    stall_timeout_seconds: float,
    current_deadline: float,
) -> float:
    saw_opening = False
    while True:
        newline = buffer.find(b"\n")
        if newline < 0:
            break
        raw = bytes(buffer[:newline])
        del buffer[: newline + 1]
        if raw.endswith(b"\r"):
            raw = raw[:-1]
        line = decode_stream(raw, "stdout line")
        if not line.startswith(OPENBENCH_PREFIX):
            raise GenerationError(f"unexpected engine stdout line: {line!r}")
        fen = line[len(OPENBENCH_PREFIX) :]
        if not fen:
            raise GenerationError("engine emitted an empty genfens FEN")
        fens.append(fen)
        if len(fens) > expected:
            raise GenerationError(f"engine emitted more than {expected} openings")
        saw_opening = True
    if saw_opening:
        return time.monotonic() + stall_timeout_seconds
    return current_deadline


def audit_fens(
    fens: list[str], *, quarantine_duplicates: bool
) -> tuple[list[str], dict[str, Any], list[int]]:
    canonical_positions: set[str] = set()
    retained_fens: list[str] = []
    duplicate_indices: list[int] = []
    turns: Counter[str] = Counter()
    fullmoves: list[int] = []
    piece_counts: list[int] = []
    for index, fen in enumerate(fens, start=1):
        if len(fen.split()) != 6:
            raise GenerationError(f"invalid FEN at opening {index}: expected six fields")
        try:
            board = chess.Board(fen)
        except ValueError as error:
            raise GenerationError(f"invalid FEN at opening {index}: {error}") from error
        if not board.is_valid():
            raise GenerationError(f"invalid FEN at opening {index}: {fen}")
        if board.is_check():
            raise GenerationError(f"opening {index} leaves the side to move in check")
        if board.is_game_over(claim_draw=False):
            raise GenerationError(f"opening {index} is terminal")
        canonical = " ".join(fen.split()[:4])
        if canonical in canonical_positions:
            if not quarantine_duplicates:
                raise GenerationError(f"duplicate canonical position at opening {index}")
            duplicate_indices.append(index)
            continue
        canonical_positions.add(canonical)
        retained_fens.append(fen)
        turns["white" if board.turn == chess.WHITE else "black"] += 1
        fullmoves.append(board.fullmove_number)
        piece_counts.append(len(board.piece_map()))

    audit = {
        "auditor": "python-chess",
        "python_chess_version": chess.__version__,
        "openings": len(retained_fens),
        "legal_nonterminal": len(retained_fens),
        "unique_canonical_positions": len(canonical_positions),
        "side_to_move": dict(sorted(turns.items())),
        "fullmove_number": {"min": min(fullmoves), "max": max(fullmoves)},
        "piece_count": {"min": min(piece_counts), "max": max(piece_counts)},
    }
    return retained_fens, audit, duplicate_indices


def publish_manifest_then_output(
    output: Path, output_bytes: bytes, manifest: Path, manifest_bytes: bytes
) -> None:
    output.parent.mkdir(parents=True, exist_ok=True)
    output_temp = output.with_name(f".{output.name}.tmp-{os.getpid()}")
    manifest_temp = manifest.with_name(f".{manifest.name}.tmp-{os.getpid()}")
    for temporary in (output_temp, manifest_temp):
        refuse_existing(temporary)
    manifest_committed = False
    output_committed = False
    try:
        write_exclusive(output_temp, output_bytes)
        write_exclusive(manifest_temp, manifest_bytes)
        link_without_overwrite(manifest_temp, manifest)
        manifest_committed = True
        link_without_overwrite(output_temp, output)
        output_committed = True
    except BaseException:
        output_temp.unlink(missing_ok=True)
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


def decode_stream(data: bytes | bytearray, label: str) -> str:
    try:
        return bytes(data).decode("utf-8")
    except UnicodeDecodeError as error:
        raise GenerationError(f"engine {label} is not valid UTF-8") from error


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


def parse_u64(value: str) -> int:
    parsed = int(value)
    if not 0 <= parsed <= U64_MAX:
        raise argparse.ArgumentTypeError("must be an unsigned 64-bit integer")
    return parsed


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
