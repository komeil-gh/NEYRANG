#!/usr/bin/env python3
"""Evaluate the sole frozen H3b vector on the sealed H3a-R1 holdout."""

from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import platform
import re
import sys
from pathlib import Path
from typing import Any

import numpy as np


REPO_ROOT = Path(__file__).resolve().parents[1]
REGISTRATION_PATH = REPO_ROOT / "docs/evidence/sanj-h3c-holdout-registration.json"
RESULT_PATH = REPO_ROOT / "docs/evidence/sanj-h3c-holdout-result.json"
COMMIT = re.compile(r"^[0-9a-f]{40}$")


def load_fitter():
    path = Path(__file__).with_name("fit-sanj-weights.py")
    spec = importlib.util.spec_from_file_location("neyrang_sanj_fitter", path)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"cannot load fitter module: {path}")
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


def canonical_bytes(value: Any) -> bytes:
    return json.dumps(
        value, sort_keys=True, separators=(",", ":"), ensure_ascii=True
    ).encode()


def repo_path(value: str) -> Path:
    path = (REPO_ROOT / value).resolve()
    try:
        path.relative_to(REPO_ROOT.resolve())
    except ValueError as error:
        raise ValueError(f"registered path escapes repository: {value}") from error
    return path


def verify_file(fitter: Any, value: dict[str, Any], *, path_key: str = "path") -> Path:
    path = repo_path(value[path_key])
    if not path.is_file():
        raise ValueError(f"registered file is missing: {value[path_key]}")
    if fitter.sha256_file(path) != value["sha256"]:
        raise ValueError(f"registered SHA-256 mismatch: {value[path_key]}")
    return path


def validate_registration_path(path: Path) -> None:
    if path.resolve() != REGISTRATION_PATH.resolve():
        raise ValueError("only the committed H3c registration may authorize access")


def validate_output_path(path: Path) -> None:
    if path.resolve() != RESULT_PATH.resolve():
        raise ValueError("H3c may write only its canonical one-time result")
    if path.exists():
        raise ValueError("H3c result already exists; a second holdout access is forbidden")


def build_decision(gates: dict[str, bool]) -> dict[str, Any]:
    accepted = all(gates.values())
    return {
        "status": (
            "accept-offline-holdout-and-authorize-h3d-registration"
            if accepted
            else "reject-h3b-and-close-h3a-r1"
        ),
        "candidate_retained": False,
        "playing_tests_authorized": accepted,
        "strength_claim": False,
        "elo_claim": False,
        "release_changed": False,
    }


def evaluate_registered(
    registration_path: Path, *, implementation_commit: str
) -> dict[str, Any]:
    validate_registration_path(registration_path)
    if COMMIT.fullmatch(implementation_commit) is None:
        raise ValueError("implementation commit must be a full lowercase Git SHA")

    registration = json.loads(registration_path.read_text(encoding="utf-8"))
    if registration.get("schema") != "neyrang-sanj-h3c-holdout-registration-v1":
        raise ValueError("unexpected H3c registration schema")
    if registration.get("status") != "binding-before-first-and-only-holdout-access":
        raise ValueError("H3c registration is not binding")

    fitter = load_fitter()
    inputs = registration["inputs"]
    holdout_spec = inputs["sealed_holdout"]
    holdout_result_path = verify_file(
        fitter,
        {
            "path": holdout_spec["result_path"],
            "sha256": holdout_spec["result_sha256"],
        },
    )
    holdout_result = json.loads(holdout_result_path.read_text(encoding="utf-8"))
    sealed = holdout_result["sealed_corpus"]
    sealed_features = sealed["artifacts"]["features"]
    if (
        holdout_result.get("schema")
        != "neyrang-sanj-h3a-final-holdout-result-v1"
        or holdout_result.get("phase") != "H3a-R1"
        or sealed_features["path"] != holdout_spec["path"]
        or sealed_features["sha256"] != holdout_spec["sha256"]
        or sealed["records"] != holdout_spec["rows_excluding_header"]
        or sealed["opening_pairs"] != holdout_spec["opening_pair_groups"]
        or sealed["trace_columns"] != holdout_spec["trace_columns"]
    ):
        raise ValueError("sealed H3a-R1 metadata differs from H3c registration")

    baseline_path = verify_file(
        fitter,
        {
            "path": inputs["baseline"]["vector_source_path"],
            "sha256": inputs["baseline"]["vector_source_sha256"],
        },
    )
    candidate_path = verify_file(
        fitter,
        {
            "path": inputs["candidate"]["result_path"],
            "sha256": inputs["candidate"]["result_sha256"],
        },
    )
    freeze_path = verify_file(
        fitter,
        {
            "path": inputs["candidate"]["source_freeze_path"],
            "sha256": inputs["candidate"]["source_freeze_sha256"],
        },
    )
    verify_file(fitter, inputs["analysis"])
    verify_file(
        fitter,
        {
            "path": inputs["analysis"]["comparison_source_path"],
            "sha256": inputs["analysis"]["comparison_source_sha256"],
        },
    )
    verify_file(
        fitter,
        {
            "path": inputs["candidate"]["binary_path"],
            "sha256": inputs["candidate"]["binary_sha256"],
        },
    )

    baseline_source = json.loads(baseline_path.read_text(encoding="utf-8"))
    candidate_result = json.loads(candidate_path.read_text(encoding="utf-8"))
    freeze = json.loads(freeze_path.read_text(encoding="utf-8"))
    if (
        baseline_source.get("schema")
        != "neyrang-sanj-h3b-fitter-registration-v1"
        or baseline_source.get("parent_git_commit")
        != inputs["baseline"]["source_git_commit"]
        or candidate_result.get("schema")
        != "neyrang-sanj-h3b-fitter-result-v1"
        or candidate_result["decision"]["status"] != "accept-one-vector"
        or freeze.get("schema") != "neyrang-sanj-h3b-source-freeze-v1"
        or freeze.get("status") != "offline-candidate-source-and-binary-frozen"
    ):
        raise ValueError("H3b evidence state differs from H3c registration")
    baseline = np.asarray(baseline_source["baseline_effective_weights"], dtype=np.int64)
    candidate = np.asarray(
        candidate_result["fit"]["vector"]["effective_weights"], dtype=np.int64
    )
    candidate_spec = inputs["candidate"]
    if fitter.stable_sha256(candidate.tolist()) != candidate_spec["vector_sha256"]:
        raise ValueError("candidate vector SHA-256 differs from H3c registration")
    if (
        freeze["provenance"]["playing_source_git_commit"]
        != candidate_spec["source_git_commit"]
        or freeze["binary"]["path"] != candidate_spec["binary_path"]
        or freeze["binary"]["sha256"] != candidate_spec["binary_sha256"]
        or freeze["provenance"]["accepted_fitter_result"]["candidate_vector_sha256"]
        != candidate_spec["vector_sha256"]
    ):
        raise ValueError("frozen H3b source or binary identity mismatch")

    scale = registration["evaluation"]["fixed_train_only_scale"]
    if candidate_result["fit"]["scale"] != scale:
        raise ValueError("H3b scale differs from the fixed H3c scale")

    holdout_path = verify_file(
        fitter, {"path": holdout_spec["path"], "sha256": holdout_spec["sha256"]}
    )
    analysis = fitter.load_analysis_module()
    holdout = analysis.load_partition(holdout_path)
    if holdout.names != tuple(baseline_source["parameter_order"]):
        raise ValueError("holdout parameter order differs from H3b registration")
    if (
        holdout.target.size != holdout_spec["rows_excluding_header"]
        or np.unique(holdout.groups).size != holdout_spec["opening_pair_groups"]
    ):
        raise ValueError("holdout row or opening-pair count mismatch")

    baseline_reconstructs = bool(
        np.array_equal(analysis.exact_integer_cp(holdout, baseline), holdout.cp)
    )
    mask = np.ones(holdout.target.size, dtype=bool)
    bootstrap = registration["evaluation"]["bootstrap"]

    def compare() -> dict[str, Any]:
        return fitter._comparison(
            analysis,
            holdout,
            mask,
            baseline,
            candidate,
            scale,
            bootstrap_seed=bootstrap["seed"],
            bootstrap_replicates=bootstrap["replicates"],
        )

    first = compare()
    second = compare()
    first_bytes = canonical_bytes(first)
    repeat_identical = first_bytes == canonical_bytes(second)
    losses = first["losses"]
    ratio = registration["acceptance_gates"][
        "candidate_maximum_absolute_cp_at_most_baseline_times"
    ]
    gates = {
        "all_registered_hashes_counts_names_and_identities_match": True,
        "baseline_exactly_reconstructs_stored_holdout_cp": baseline_reconstructs,
        "candidate_vector_hash_matches_frozen_h3b": True,
        "cross_entropy_point_delta_strictly_below_zero": (
            losses["cross_entropy"]["delta"] < 0.0
        ),
        "mse_point_delta_strictly_below_zero": losses["mse"]["delta"] < 0.0,
        "cross_entropy_bootstrap_upper_endpoint_strictly_below_zero": (
            losses["cross_entropy"]["group_bootstrap_delta"]["upper"] < 0.0
        ),
        "mse_bootstrap_upper_endpoint_strictly_below_zero": (
            losses["mse"]["group_bootstrap_delta"]["upper"] < 0.0
        ),
        "candidate_cp_range_within_registered_ratio": (
            first["maximum_absolute_cp"]["candidate"]
            <= ratio * first["maximum_absolute_cp"]["baseline"]
        ),
        "two_core_evaluations_byte_identical": repeat_identical,
    }
    return {
        "schema": "neyrang-sanj-h3c-holdout-result-v1",
        "phase": "H3c",
        "implementation_git_commit": implementation_commit,
        "registration": {
            "path": str(registration_path.relative_to(REPO_ROOT)),
            "sha256": fitter.sha256_file(registration_path),
        },
        "implementation": {
            "path": str(Path(__file__).resolve().relative_to(REPO_ROOT)),
            "sha256": fitter.sha256_file(Path(__file__).resolve()),
            "python": platform.python_version(),
            "numpy": np.__version__,
            "platform": platform.platform(),
            "machine": platform.machine(),
        },
        "inputs": {
            "holdout_sha256": holdout_spec["sha256"],
            "rows": int(holdout.target.size),
            "opening_pair_groups": int(np.unique(holdout.groups).size),
            "baseline_vector_sha256": fitter.stable_sha256(baseline.tolist()),
            "candidate_vector_sha256": fitter.stable_sha256(candidate.tolist()),
            "fixed_train_only_scale": scale,
        },
        "holdout": first,
        "determinism": {
            "complete_in_memory_evaluations": 2,
            "byte_identical_core_results": repeat_identical,
            "core_sha256": hashlib.sha256(first_bytes).hexdigest(),
        },
        "gates": gates,
        "decision": build_decision(gates),
    }


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--registration", type=Path, required=True)
    parser.add_argument("--implementation-commit", required=True)
    parser.add_argument("--output", type=Path, required=True)
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    validate_output_path(args.output)
    result = evaluate_registered(
        args.registration.resolve(),
        implementation_commit=args.implementation_commit,
    )
    with args.output.open("x", encoding="utf-8") as handle:
        json.dump(result, handle, indent=2, sort_keys=True)
        handle.write("\n")
    print(json.dumps(result["decision"], sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
