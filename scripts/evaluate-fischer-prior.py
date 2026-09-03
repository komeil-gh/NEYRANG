#!/usr/bin/env python3
"""Evaluate a frozen Fischer prior without fitting or modifying it."""

from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import os
import sys
from array import array
from datetime import datetime, timezone
from pathlib import Path
from types import ModuleType

FITTER = Path(__file__).with_name("fit-fischer-prior.py")
SCHEMA = "neyrang-fischer-prior-evaluation-v1"


class EvaluationError(RuntimeError):
    """A fail-closed artifact evaluation error."""


def load_fitter() -> ModuleType:
    spec = importlib.util.spec_from_file_location("neyrang_fischer_fitter", FITTER)
    if spec is None or spec.loader is None:
        raise EvaluationError(f"cannot load fitter: {FITTER}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def sha256_file(path: Path) -> tuple[str, int]:
    digest = hashlib.sha256()
    size = 0
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
            size += len(chunk)
    return digest.hexdigest(), size


def load_tables(path: Path, fitter: ModuleType) -> tuple[array, array]:
    payload = path.read_bytes()
    expected_size = fitter.HEADER.size + 2 * (fitter.FROM_TO_LEN + fitter.PIECE_TO_LEN)
    if len(payload) != expected_size:
        raise EvaluationError(f"artifact is {len(payload)} bytes, expected {expected_size}")
    header = fitter.HEADER.unpack_from(payload)
    expected_header = (
        fitter.MAGIC,
        fitter.VERSION,
        fitter.PHASES,
        fitter.PIECES,
        fitter.SQUARES,
        fitter.FROM_TO_LEN,
        fitter.PIECE_TO_LEN,
    )
    if header != expected_header:
        raise EvaluationError("artifact header does not match the frozen format")
    weights = array("h")
    weights.frombytes(payload[fitter.HEADER.size :])
    if sys.byteorder != "little":
        weights.byteswap()
    return weights[: fitter.FROM_TO_LEN], weights[fitter.FROM_TO_LEN :]


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--labels", required=True, type=Path)
    parser.add_argument("--artifact", required=True, type=Path)
    parser.add_argument("--artifact-sha256", required=True)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--max-cp-loss", type=int, default=50)
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    try:
        labels = args.labels.resolve()
        artifact = args.artifact.resolve()
        output = args.output.resolve()
        if not labels.is_file() or not artifact.is_file():
            raise EvaluationError("labels and artifact must both be files")
        if output.exists():
            raise EvaluationError(f"refusing to overwrite output: {output}")
        if args.max_cp_loss < 0:
            raise EvaluationError("max-cp-loss must be non-negative")
        artifact_hash, artifact_bytes = sha256_file(artifact)
        if artifact_hash != args.artifact_sha256.casefold():
            raise EvaluationError("artifact SHA-256 does not match the required identity")
        labels_hash, labels_bytes = sha256_file(labels)
        fitter = load_fitter()
        metrics = fitter.evaluate(labels, load_tables(artifact, fitter), args.max_cp_loss)
        metrics["uniform_top1_lift"] = (
            metrics["top1"] / metrics["uniform_top1_expectation"]
        )
        report = {
            "schema": SCHEMA,
            "created_at": datetime.now(timezone.utc).isoformat(),
            "labels": {
                "path": str(labels),
                "bytes": labels_bytes,
                "sha256": labels_hash,
            },
            "artifact": {
                "path": str(artifact),
                "bytes": artifact_bytes,
                "sha256": artifact_hash,
            },
            "max_cp_loss": args.max_cp_loss,
            "metrics": metrics,
        }
        output.parent.mkdir(parents=True, exist_ok=True)
        temporary = output.with_name(f".{output.name}.tmp-{os.getpid()}")
        temporary.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n")
        temporary.replace(output)
    except (EvaluationError, OSError, ValueError) as error:
        print(f"evaluate-fischer-prior: {error}", file=sys.stderr)
        return 1
    print(json.dumps(metrics, sort_keys=True))
    print(f"report: {output}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
