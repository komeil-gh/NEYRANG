#!/usr/bin/env python3
"""Fit one registered, bounded SANJ weight candidate without holdout access."""

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
REGISTRATION_PATH = REPO_ROOT / "testing/private/sanj-h3b-fitter-registration.json"
COMMIT = re.compile(r"^[0-9a-f]{40}$")


def load_analysis_module():
    path = Path(__file__).with_name("analyze-sanj-features.py")
    spec = importlib.util.spec_from_file_location("neyrang_sanj_analysis", path)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"cannot load analysis module: {path}")
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def stable_sha256(value: Any) -> str:
    encoded = json.dumps(
        value, sort_keys=True, separators=(",", ":"), ensure_ascii=True
    ).encode()
    return hashlib.sha256(encoded).hexdigest()


def validate_feature_path(path: Path, *, expected_name: str) -> None:
    lowered = [part.lower() for part in path.parts]
    if any("holdout" in part for part in lowered):
        raise ValueError("holdout paths are forbidden in H3b")
    if path.name != expected_name:
        raise ValueError(f"expected feature filename {expected_name!r}")


def round_half_away_from_zero(values: np.ndarray) -> np.ndarray:
    values = np.asarray(values, dtype=np.float64)
    rounded = np.where(
        values >= 0.0, np.floor(values + 0.5), np.ceil(values - 0.5)
    )
    return rounded.astype(np.int64)


def inner_group_split(
    groups: np.ndarray, seed: str, calibration_fraction: float
) -> tuple[np.ndarray, np.ndarray]:
    if not 0.0 < calibration_fraction < 1.0:
        raise ValueError("calibration fraction must be inside (0,1)")
    unique = np.unique(groups)
    if unique.size < 2:
        raise ValueError("at least two opening groups are required")
    ranked = sorted(
        unique.tolist(),
        key=lambda group: (
            hashlib.sha256(f"{seed}\t{group}".encode()).digest(),
            group,
        ),
    )
    calibration_count = max(1, int(np.floor(unique.size * calibration_fraction + 0.5)))
    if calibration_count >= unique.size:
        calibration_count = unique.size - 1
    calibration_groups = np.asarray(ranked[:calibration_count], dtype=object)
    calibration = np.isin(groups, calibration_groups)
    return ~calibration, calibration


def objective_and_gradient(
    candidate: np.ndarray,
    design: np.ndarray,
    target: np.ndarray,
    row_weights: np.ndarray,
    baseline: np.ndarray,
    radii: np.ndarray,
    *,
    scale: float,
    regularization: float,
) -> tuple[float, np.ndarray]:
    candidate = np.asarray(candidate, dtype=np.float64)
    exponent = np.clip(-scale * (design @ candidate) / 400.0, -50.0, 50.0)
    prediction = 1.0 / (1.0 + np.power(10.0, exponent))
    clipped = np.clip(prediction, 1e-12, 1.0 - 1e-12)
    cross_entropy = -(
        target * np.log(clipped) + (1.0 - target) * np.log(1.0 - clipped)
    )
    delta = candidate - baseline
    penalty = regularization * float(np.mean(np.square(delta / radii)))
    value = float(np.dot(row_weights, cross_entropy)) + penalty
    slope = np.log(10.0) * scale / 400.0
    gradient = design.T @ (row_weights * (prediction - target) * slope)
    gradient += (
        2.0 * regularization * delta / np.square(radii) / candidate.size
    )
    return value, gradient


def effective_to_source_literals(weights: np.ndarray) -> dict[str, Any]:
    w = np.asarray(weights, dtype=np.int64)
    if w.shape != (42,):
        raise ValueError("expected 42 effective weights")
    return {
        "mg_material": [
            int(w[0]),
            int(w[1] + 24),
            int(w[2] + 12),
            int(w[3]),
            int(w[4] + 5),
            0,
        ],
        "eg_material": [
            int(w[25]),
            int(w[26] + 18),
            int(w[27] + 10),
            int(w[28]),
            int(w[29]),
            0,
        ],
        "mg_psqt": [int(value) for value in w[5:14]],
        "eg_psqt": [int(value) for value in w[30:37]],
        "mg_terms": {
            "bishop_pair": int(w[14]),
            "doubled_extra": int(w[15]),
            "isolated_pawn": int(w[16]),
            "passed_rank_sq": int(w[17]),
            "mobility": [int(value) for value in w[18:22]],
            "rook_open": int(w[22]),
            "rook_semi_open": int(w[23]),
            "king_shield": int(w[24]),
        },
        "eg_terms": {
            "bishop_pair": int(w[37]),
            "doubled_extra": int(w[38]),
            "isolated_pawn": int(w[39]),
            "passed_rank_sq": int(w[40]),
        },
        "tempo": int(w[41]),
        "fixed_psqt_intercepts": {
            "mg_knight": -24,
            "eg_knight": -18,
            "mg_bishop": -12,
            "eg_bishop": -10,
            "mg_queen": -5,
            "eg_king": -20,
        },
    }


def material_order_is_safe(weights: np.ndarray) -> bool:
    w = np.asarray(weights)
    for material in (w[:5], w[25:30]):
        pawn, knight, bishop, rook, queen = material.tolist()
        if not queen > rook > max(knight, bishop) > pawn > 0:
            return False
    return True


def build_decision(accepted: bool) -> dict[str, Any]:
    return {
        "status": "accept-one-vector" if accepted else "reject-vector",
        "candidate_frozen": accepted,
        "holdout_access_authorized": False,
        "weight_source_changed": False,
        "strength_claim": False,
        "next_gate": (
            "apply exactly this vector to registered SANJ literals, commit "
            "source, freeze binary, then separately authorize H3c"
            if accepted
            else "retain fitter infrastructure; do not change SANJ weights "
            "or open H3a-R1"
        ),
    }


def _fit_one(
    partition: Any,
    mask: np.ndarray,
    baseline: np.ndarray,
    lower: np.ndarray,
    upper: np.ndarray,
    radii: np.ndarray,
    *,
    scale: float,
    regularization: float,
    optimizer: dict[str, Any],
) -> tuple[np.ndarray, dict[str, Any]]:
    from scipy.optimize import minimize

    design = partition.design[mask]
    target = partition.target[mask]
    row_weights = load_analysis_module().group_weights(partition.groups[mask])

    def objective(candidate: np.ndarray) -> tuple[float, np.ndarray]:
        return objective_and_gradient(
            candidate,
            design,
            target,
            row_weights,
            baseline,
            radii,
            scale=scale,
            regularization=regularization,
        )

    result = minimize(
        objective,
        baseline.astype(np.float64),
        method="L-BFGS-B",
        jac=True,
        bounds=list(zip(lower.tolist(), upper.tolist(), strict=True)),
        options={
            "maxiter": optimizer["maxiter"],
            "maxfun": optimizer["maxfun"],
            "maxls": optimizer["maxls"],
            "maxcor": optimizer["maxcor"],
            "ftol": optimizer["ftol"],
            "gtol": optimizer["gtol"],
        },
    )
    summary = {
        "success": bool(result.success),
        "status": int(result.status),
        "message": str(result.message),
        "iterations": int(result.nit),
        "function_evaluations": int(result.nfev),
        "final_objective": float(result.fun),
        "maximum_absolute_projected_jacobian": float(np.max(np.abs(result.jac))),
    }
    return np.asarray(result.x, dtype=np.float64), summary


def _comparison(
    analysis: Any,
    partition: Any,
    mask: np.ndarray,
    baseline: np.ndarray,
    candidate: np.ndarray,
    scale: float,
    *,
    bootstrap_seed: int | None = None,
    bootstrap_replicates: int | None = None,
) -> dict[str, Any]:
    baseline_cp = analysis.exact_integer_cp(partition, baseline)[mask]
    candidate_cp = analysis.exact_integer_cp(partition, candidate)[mask]
    target = partition.target[mask]
    groups = partition.groups[mask]
    weights = analysis.group_weights(groups)
    output: dict[str, Any] = {
        "rows": int(mask.sum()),
        "groups": int(np.unique(groups).size),
        "maximum_absolute_cp": {
            "baseline": int(np.max(np.abs(baseline_cp))),
            "candidate": int(np.max(np.abs(candidate_cp))),
        },
        "losses": {},
    }
    for kind in ("cross_entropy", "mse"):
        baseline_values = analysis.loss_values(
            target, analysis.probability(baseline_cp, scale), kind
        )
        candidate_values = analysis.loss_values(
            target, analysis.probability(candidate_cp, scale), kind
        )
        baseline_loss = float(np.dot(weights, baseline_values))
        candidate_loss = float(np.dot(weights, candidate_values))
        loss: dict[str, Any] = {
            "baseline": baseline_loss,
            "candidate": candidate_loss,
            "delta": candidate_loss - baseline_loss,
        }
        if bootstrap_seed is not None and bootstrap_replicates is not None:
            _, group_deltas = analysis.group_mean_values(
                groups, candidate_values - baseline_values
            )
            loss["group_bootstrap_delta"] = analysis.bootstrap_group_delta(
                group_deltas,
                seed=bootstrap_seed,
                replicates=bootstrap_replicates,
            )
        output["losses"][kind] = loss
    return output


def _load_registration() -> dict[str, Any]:
    registration = json.loads(REGISTRATION_PATH.read_text(encoding="utf-8"))
    if registration.get("schema") != "neyrang-sanj-h3b-fitter-registration-v1":
        raise ValueError("unexpected H3b registration schema")
    if registration.get("status") != "binding-before-real-fit":
        raise ValueError("H3b registration is not binding")
    return registration


def _verify_partition(
    analysis: Any,
    partition: Any,
    expected: dict[str, Any],
    baseline: np.ndarray,
) -> None:
    if partition.target.size != expected["rows_excluding_header"]:
        raise ValueError("registered feature row count mismatch")
    if np.unique(partition.groups).size != expected["opening_pair_groups"]:
        raise ValueError("registered opening-group count mismatch")
    reconstructed = analysis.exact_integer_cp(partition, baseline)
    if not np.array_equal(reconstructed, partition.cp):
        raise ValueError("baseline vector does not exactly reconstruct trace cp")


def _fit_once(
    registration: dict[str, Any], train: Any, validation: Any, analysis: Any
) -> dict[str, Any]:
    baseline = np.asarray(registration["baseline_effective_weights"], dtype=np.float64)
    baseline_integer = baseline.astype(np.int64)
    lower = np.asarray(registration["lower_bounds"], dtype=np.float64)
    upper = np.asarray(registration["upper_bounds"], dtype=np.float64)
    radii = np.asarray(registration["regularization_radii"], dtype=np.float64)
    fit = registration["fit"]
    scale = analysis.fit_scale(
        train.target,
        train.cp,
        analysis.group_weights(train.groups),
        "cross_entropy",
    )
    inner_train, inner_calibration = inner_group_split(
        train.groups,
        fit["inner_split"]["seed"],
        fit["inner_split"]["calibration_fraction"],
    )
    trials = []
    for regularization in fit["lambda_grid"]:
        continuous, optimizer = _fit_one(
            train,
            inner_train,
            baseline,
            lower,
            upper,
            radii,
            scale=scale,
            regularization=regularization,
            optimizer=fit["optimizer"],
        )
        rounded = round_half_away_from_zero(continuous)
        calibration = _comparison(
            analysis,
            train,
            inner_calibration,
            baseline_integer,
            rounded,
            scale,
        )
        trials.append(
            {
                "lambda": regularization,
                "optimizer": optimizer,
                "rounded_vector_sha256": stable_sha256(rounded.tolist()),
                "inner_calibration": calibration,
                "continuous_maximum_absolute_delta": float(
                    np.max(np.abs(continuous - baseline))
                ),
            }
        )
    best_loss = min(
        trial["inner_calibration"]["losses"]["cross_entropy"]["candidate"]
        for trial in trials
    )
    eligible = [
        trial
        for trial in trials
        if trial["inner_calibration"]["losses"]["cross_entropy"]["candidate"]
        <= best_loss + 1e-12
    ]
    selected = max(eligible, key=lambda trial: trial["lambda"])
    selected_lambda = selected["lambda"]
    continuous, final_optimizer = _fit_one(
        train,
        np.ones(train.target.size, dtype=bool),
        baseline,
        lower,
        upper,
        radii,
        scale=scale,
        regularization=selected_lambda,
        optimizer=fit["optimizer"],
    )
    rounded = round_half_away_from_zero(continuous)
    train_comparison = _comparison(
        analysis,
        train,
        np.ones(train.target.size, dtype=bool),
        baseline_integer,
        rounded,
        scale,
    )
    uncertainty = registration["uncertainty"]
    validation_comparison = _comparison(
        analysis,
        validation,
        np.ones(validation.target.size, dtype=bool),
        baseline_integer,
        rounded,
        scale,
        bootstrap_seed=uncertainty["seed"],
        bootstrap_replicates=uncertainty["replicates"],
    )
    optimizer_success = all(trial["optimizer"]["success"] for trial in trials)
    optimizer_success = optimizer_success and final_optimizer["success"]
    in_bounds = bool(np.all(rounded >= lower) and np.all(rounded <= upper))
    train_improves = all(
        train_comparison["losses"][kind]["delta"] < 0.0
        for kind in ("cross_entropy", "mse")
    )
    validation_improves = all(
        validation_comparison["losses"][kind]["delta"] < 0.0
        for kind in ("cross_entropy", "mse")
    )
    bootstrap_resolves = all(
        validation_comparison["losses"][kind]["group_bootstrap_delta"]["upper"]
        < 0.0
        for kind in ("cross_entropy", "mse")
    )
    cp_range_safe = all(
        comparison["maximum_absolute_cp"]["candidate"]
        <= 1.25 * comparison["maximum_absolute_cp"]["baseline"]
        for comparison in (train_comparison, validation_comparison)
    )
    gates = {
        "all_optimizers_success": optimizer_success,
        "rounded_vector_in_bounds": in_bounds,
        "material_order_safe": material_order_is_safe(rounded),
        "train_both_losses_improve": train_improves,
        "validation_both_point_losses_improve": validation_improves,
        "validation_both_bootstrap_upper_endpoints_below_zero": bootstrap_resolves,
        "cp_range_within_registered_ratio": cp_range_safe,
    }
    passed = all(gates.values())
    vector_payload: dict[str, Any]
    if passed:
        vector_payload = {
            "effective_weights": rounded.tolist(),
            "effective_delta": (rounded - baseline_integer).tolist(),
            "effective_weights_sha256": stable_sha256(rounded.tolist()),
            "source_literals": effective_to_source_literals(rounded),
        }
    else:
        vector_payload = {
            "rejected_vector_sha256": stable_sha256(rounded.tolist()),
            "changed_parameter_count": int(
                np.count_nonzero(rounded - baseline_integer)
            ),
            "maximum_absolute_integer_delta": int(
                np.max(np.abs(rounded - baseline_integer))
            ),
        }
    return {
        "scale": scale,
        "inner_split": {
            "fit_rows": int(inner_train.sum()),
            "fit_groups": int(np.unique(train.groups[inner_train]).size),
            "calibration_rows": int(inner_calibration.sum()),
            "calibration_groups": int(
                np.unique(train.groups[inner_calibration]).size
            ),
        },
        "lambda_trials": trials,
        "selected_lambda": selected_lambda,
        "final_optimizer": final_optimizer,
        "train": train_comparison,
        "validation": validation_comparison,
        "gates": gates,
        "pre_repeat_passed": passed,
        "vector": vector_payload,
    }


def fit_registered(
    train_path: Path,
    validation_path: Path,
    *,
    implementation_commit: str,
) -> dict[str, Any]:
    if COMMIT.fullmatch(implementation_commit) is None:
        raise ValueError("implementation commit must be a full lowercase Git SHA")
    validate_feature_path(train_path, expected_name="train.features.tsv")
    validate_feature_path(
        validation_path, expected_name="validation.features.tsv"
    )
    registration = _load_registration()
    registered_train = REPO_ROOT / registration["inputs"]["train"]["path"]
    registered_validation = REPO_ROOT / registration["inputs"]["validation"]["path"]
    if train_path.resolve() != registered_train.resolve():
        raise ValueError("train path is not the registered H2e input")
    if validation_path.resolve() != registered_validation.resolve():
        raise ValueError("validation path is not the registered H2e input")
    if sha256_file(train_path) != registration["inputs"]["train"]["sha256"]:
        raise ValueError("train SHA-256 mismatch")
    if (
        sha256_file(validation_path)
        != registration["inputs"]["validation"]["sha256"]
    ):
        raise ValueError("validation SHA-256 mismatch")
    analysis_path = Path(__file__).with_name("analyze-sanj-features.py")
    if sha256_file(analysis_path) != registration["inputs"]["loader"]["sha256"]:
        raise ValueError("registered loader SHA-256 mismatch")
    analysis = load_analysis_module()
    baseline = np.asarray(registration["baseline_effective_weights"], dtype=np.int64)
    if tuple(baseline.tolist()) != tuple(analysis.CURRENT_EFFECTIVE_WEIGHTS):
        raise ValueError("registered baseline differs from loader baseline")
    train = analysis.load_partition(train_path)
    validation = analysis.load_partition(validation_path)
    if train.names != tuple(registration["parameter_order"]):
        raise ValueError("train parameter order differs from registration")
    if validation.names != train.names:
        raise ValueError("validation parameter order differs from train")
    if np.intersect1d(np.unique(train.groups), np.unique(validation.groups)).size:
        raise ValueError("train and validation opening groups overlap")
    _verify_partition(
        analysis, train, registration["inputs"]["train"], baseline
    )
    _verify_partition(
        analysis,
        validation,
        registration["inputs"]["validation"],
        baseline,
    )
    first = _fit_once(registration, train, validation, analysis)
    second = _fit_once(registration, train, validation, analysis)
    first_bytes = json.dumps(
        first, sort_keys=True, separators=(",", ":"), ensure_ascii=True
    ).encode()
    second_bytes = json.dumps(
        second, sort_keys=True, separators=(",", ":"), ensure_ascii=True
    ).encode()
    repeat_identical = first_bytes == second_bytes
    accepted = first["pre_repeat_passed"] and repeat_identical
    if not repeat_identical:
        first["vector"] = {
            "rejected_vector_sha256": stable_sha256(first.get("vector", {}))
        }
    import scipy

    return {
        "schema": "neyrang-sanj-h3b-fitter-result-v1",
        "phase": "H3b",
        "implementation_git_commit": implementation_commit,
        "registration": {
            "path": str(REGISTRATION_PATH.relative_to(REPO_ROOT)),
            "sha256": sha256_file(REGISTRATION_PATH),
        },
        "implementation": {
            "fitter_path": str(Path(__file__).resolve().relative_to(REPO_ROOT)),
            "fitter_sha256": sha256_file(Path(__file__).resolve()),
            "loader_sha256": sha256_file(analysis_path),
            "python": platform.python_version(),
            "numpy": np.__version__,
            "scipy": scipy.__version__,
            "platform": platform.platform(),
            "machine": platform.machine(),
        },
        "inputs": {
            "train_sha256": sha256_file(train_path),
            "validation_sha256": sha256_file(validation_path),
            "train_rows": int(train.target.size),
            "validation_rows": int(validation.target.size),
            "train_groups": int(np.unique(train.groups).size),
            "validation_groups": int(np.unique(validation.groups).size),
            "group_overlap": 0,
            "baseline_reconstruction_mismatches": 0,
        },
        "determinism": {
            "complete_fit_repetitions": 2,
            "byte_identical_core_results": repeat_identical,
            "core_sha256": hashlib.sha256(first_bytes).hexdigest(),
        },
        "fit": first,
        "decision": build_decision(accepted),
    }


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--train-features", type=Path, required=True)
    parser.add_argument("--validation-features", type=Path, required=True)
    parser.add_argument("--implementation-commit", required=True)
    parser.add_argument("--output", type=Path, required=True)
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    report = fit_registered(
        args.train_features,
        args.validation_features,
        implementation_commit=args.implementation_commit,
    )
    args.output.parent.mkdir(parents=True, exist_ok=True)
    with args.output.open("x", encoding="utf-8", newline="\n") as handle:
        json.dump(report, handle, indent=2, sort_keys=True)
        handle.write("\n")
    print(json.dumps(report["decision"], sort_keys=True))
    return 0 if report["decision"]["candidate_frozen"] else 2


if __name__ == "__main__":
    raise SystemExit(main())
