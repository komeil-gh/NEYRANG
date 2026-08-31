#!/usr/bin/env python3
"""Prepare and verify immutable shards for distributed NEYRANG matches."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import platform
import re
import subprocess
import sys
from dataclasses import dataclass
from decimal import Decimal, InvalidOperation
from pathlib import Path
from typing import Any

import chess


TIME_CONTROL = re.compile(
    r"^(?:(?P<moves>[1-9][0-9]*)/)?"
    r"(?P<base>[0-9]+(?:\.[0-9]+)?)"
    r"(?:\+(?P<increment>[0-9]+(?:\.[0-9]+)?))?$"
)


class CampaignError(ValueError):
    """A distributed campaign violated its fail-closed contract."""


@dataclass(frozen=True)
class CampaignConfig:
    engine_a: Path
    engine_a_name: str
    engine_a_git_sha: str
    engine_b: Path
    engine_b_name: str
    engine_b_git_sha: str
    fastchess: Path
    fastchess_version: str
    openings: Path
    openings_source: str
    openings_license: str
    output: Path
    seed: str
    total_pairs: int
    pairs_per_shard: int
    time_control: str
    hash_mb: int
    threads: int
    move_overhead_ms: int
    time_margin_ms: int
    strict: bool
    warning_policy: str


def prepare_campaign(config: CampaignConfig) -> dict[str, Any]:
    _validate_config(config)
    assets = {
        "engine_a": _asset(
            config.engine_a, config.engine_a_name, config.engine_a_git_sha
        ),
        "engine_b": _asset(
            config.engine_b, config.engine_b_name, config.engine_b_git_sha
        ),
        "fastchess": {
            "filename": config.fastchess.name,
            "sha256": sha256_file(config.fastchess),
            "version": config.fastchess_version,
        },
    }
    source_lines = _read_unique_openings(config.openings)
    if len(source_lines) < config.total_pairs:
        raise CampaignError(
            f"requested {config.total_pairs} pairs but only "
            f"{len(source_lines)} unique openings are available"
        )

    ranked = sorted(
        source_lines,
        key=lambda item: (
            hashlib.sha256(
                config.seed.encode("utf-8") + b"\0" + item[0].encode("utf-8")
            ).digest(),
            item[0],
        ),
    )[: config.total_pairs]
    match = {
        "time_control": canonical_time_control(config.time_control),
        "hash_mb": config.hash_mb,
        "threads": config.threads,
        "move_overhead_ms": config.move_overhead_ms,
        "time_margin_ms": config.time_margin_ms,
        "concurrency": 1,
        "opening_order": "sequential",
        "strict": config.strict,
        "warning_policy": config.warning_policy,
        "adjudication": "fastchess-default",
    }
    shard_openings: list[tuple[dict[str, Any], bytes]] = []
    for index, offset in enumerate(range(0, config.total_pairs, config.pairs_per_shard)):
        selected = ranked[offset : offset + config.pairs_per_shard]
        opening_bytes = ("\n".join(item[1] for item in selected) + "\n").encode()
        shard_id = f"shard-{index:04}"
        descriptor = {
            "shard_id": shard_id,
            "index": index,
            "pair_offset": offset,
            "pairs": len(selected),
            "games": len(selected) * 2,
            "openings_file": f"openings/{shard_id}.epd",
            "openings_sha256": sha256_bytes(opening_bytes),
        }
        shard_openings.append((descriptor, opening_bytes))

    campaign_core = {
        "format": "neyrang-distributed-campaign-v1",
        "assets": assets,
        "source_openings": {
            "filename": config.openings.name,
            "sha256": sha256_file(config.openings),
            "source": config.openings_source,
            "license": config.openings_license,
            "unique_positions": len(source_lines),
        },
        "selection": {
            "seed": config.seed,
            "pairs": config.total_pairs,
            "games": config.total_pairs * 2,
            "pairs_per_shard": config.pairs_per_shard,
        },
        "required_platform": {
            "os": platform.system(),
            "architecture": platform.machine(),
        },
        "match": match,
        "shards": [descriptor for descriptor, _ in shard_openings],
    }
    campaign_id = sha256_bytes(canonical_json(campaign_core))

    openings_dir = config.output / "openings"
    shards_dir = config.output / "shards"
    openings_dir.mkdir(parents=True)
    shards_dir.mkdir()
    campaign_shards: list[dict[str, Any]] = []
    for descriptor, opening_bytes in shard_openings:
        openings_path = config.output / descriptor["openings_file"]
        openings_path.write_bytes(opening_bytes)
        shard_manifest = {
            "format": "neyrang-distributed-shard-v1",
            "campaign_id": campaign_id,
            **descriptor,
            "assets": assets,
            "required_platform": campaign_core["required_platform"],
            "match": match,
            "outputs": {
                "pgn": f"results/{descriptor['shard_id']}.pgn",
                "log": f"results/{descriptor['shard_id']}.log",
                "metadata": f"results/{descriptor['shard_id']}.meta.txt",
                "config": f"results/{descriptor['shard_id']}.config.json",
            },
        }
        manifest_path = shards_dir / f"{descriptor['shard_id']}.json"
        manifest_bytes = pretty_json(shard_manifest)
        manifest_path.write_bytes(manifest_bytes)
        campaign_shards.append(
            {
                **descriptor,
                "manifest_file": f"shards/{manifest_path.name}",
                "manifest_sha256": sha256_bytes(manifest_bytes),
            }
        )

    campaign = {
        **campaign_core,
        "campaign_id": campaign_id,
        "shards": campaign_shards,
    }
    campaign_bytes = pretty_json(campaign)
    (config.output / "campaign.json").write_bytes(campaign_bytes)
    return {
        "campaign_id": campaign_id,
        "campaign_sha256": sha256_bytes(campaign_bytes),
        "pairs": config.total_pairs,
        "games": config.total_pairs * 2,
        "shards": len(campaign_shards),
        "output": str(config.output),
    }


def verify_shard_assets(
    manifest: Path,
    *,
    engine_a: Path,
    engine_b: Path,
    fastchess: Path,
    campaign_sha256: str,
) -> dict[str, Any]:
    campaign_root = manifest.parent.parent
    campaign, data, _, actual_manifest_sha256 = _load_bound_shard(
        manifest, campaign_sha256
    )
    required_platform = campaign.get("required_platform", {})
    actual_platform = {"os": platform.system(), "architecture": platform.machine()}
    if required_platform != actual_platform:
        raise CampaignError(
            f"worker platform is {actual_platform}, expected {required_platform}"
        )
    openings = campaign_root / data["openings_file"]
    expected = {
        "engine_a": (engine_a, data["assets"]["engine_a"]["sha256"]),
        "engine_b": (engine_b, data["assets"]["engine_b"]["sha256"]),
        "fastchess": (fastchess, data["assets"]["fastchess"]["sha256"]),
        "openings": (openings, data["openings_sha256"]),
    }
    verified: dict[str, str] = {}
    for name, (path, expected_sha256) in expected.items():
        actual = sha256_file(path)
        if actual != expected_sha256:
            raise CampaignError(
                f"{name} SHA-256 is {actual}, expected {expected_sha256}"
            )
        verified[name] = actual
    return {
        "campaign_id": data["campaign_id"],
        "shard_id": data["shard_id"],
        "pairs": data["pairs"],
        "games": data["games"],
        "campaign_sha256": campaign_sha256,
        "shard_manifest_sha256": actual_manifest_sha256,
        "verified_sha256": verified,
    }


def build_shard_environment(
    manifest: Path,
    *,
    engine_a: Path,
    engine_b: Path,
    fastchess: Path,
    results: Path,
    campaign_sha256: str,
) -> dict[str, str]:
    verified = verify_shard_assets(
        manifest,
        engine_a=engine_a,
        engine_b=engine_b,
        fastchess=fastchess,
        campaign_sha256=campaign_sha256,
    )
    campaign_root = manifest.parent.parent
    campaign = json.loads((campaign_root / "campaign.json").read_text(encoding="utf-8"))
    shard = json.loads(manifest.read_text(encoding="utf-8"))
    match = shard["match"]
    assets = shard["assets"]
    shard_id = shard["shard_id"]
    seed = (int(shard["campaign_id"][:16], 16) ^ int(shard["index"])) % 2_147_483_647
    return {
        "FASTCHESS_BIN": str(fastchess.resolve()),
        "ENGINE_A": str(engine_a.resolve()),
        "ENGINE_A_NAME": assets["engine_a"]["name"],
        "ENGINE_A_GIT_SHA": assets["engine_a"]["git_sha"],
        "ENGINE_B": str(engine_b.resolve()),
        "ENGINE_B_NAME": assets["engine_b"]["name"],
        "ENGINE_B_GIT_SHA": assets["engine_b"]["git_sha"],
        "OPENINGS_FILE": str((campaign_root / shard["openings_file"]).resolve()),
        "OPENINGS_SOURCE": campaign["source_openings"]["source"],
        "OPENINGS_LICENSE": campaign["source_openings"]["license"],
        "OPENING_ORDER": "sequential",
        "OPENING_SEED": str(seed),
        "GAMES": str(shard["games"]),
        "TC": match["time_control"],
        "HASH_MB": str(match["hash_mb"]),
        "THREADS": str(match["threads"]),
        "MOVE_OVERHEAD_MS": str(match["move_overhead_ms"]),
        "TIME_MARGIN_MS": str(match["time_margin_ms"]),
        "CONCURRENCY": str(match["concurrency"]),
        "STRICT": "1" if match["strict"] else "0",
        "WARNING_POLICY": match["warning_policy"],
        "SHOW_LATENCY": "1",
        "AUTOSAVE_INTERVAL": "20",
        "CAMPAIGN_ID": shard["campaign_id"],
        "SHARD_ID": shard_id,
        "CAMPAIGN_MANIFEST_SHA256": verified["campaign_sha256"],
        "SHARD_MANIFEST_SHA256": verified["shard_manifest_sha256"],
        "PAIR_OFFSET": str(shard["pair_offset"]),
        "PGN_OUT": str(results / f"{shard_id}.pgn"),
        "LOG_OUT": str(results / f"{shard_id}.log"),
        "META_OUT": str(results / f"{shard_id}.meta.txt"),
        "CONFIG_OUT": str(results / f"{shard_id}.config.json"),
    }


def finalize_shard_result(
    manifest: Path, *, results: Path, campaign_sha256: str
) -> dict[str, Any]:
    campaign, shard, _, manifest_sha256 = _load_bound_shard(
        manifest, campaign_sha256
    )
    shard_id = shard["shard_id"]
    result_path = results / f"{shard_id}.result.json"
    if result_path.exists():
        raise CampaignError(f"refusing to overwrite existing result {result_path}")
    filenames = {
        "pgn": f"{shard_id}.pgn",
        "log": f"{shard_id}.log",
        "metadata": f"{shard_id}.meta.txt",
        "config": f"{shard_id}.config.json",
        "audit": f"{shard_id}.audit.json",
    }
    artifacts: dict[str, dict[str, Any]] = {}
    for label, filename in filenames.items():
        path = results / filename
        if not path.is_file():
            raise CampaignError(f"missing {label} artifact: {path}")
        artifacts[label] = {
            "file": filename,
            "bytes": path.stat().st_size,
            "sha256": sha256_file(path),
        }
    audit = json.loads((results / filenames["audit"]).read_text(encoding="utf-8"))
    if audit.get("format") != "neyrang-match-audit-v1" or audit.get("ok") is not True:
        raise CampaignError("shard audit is not a successful neyrang-match-audit-v1 result")
    if audit.get("games") != shard["games"] or audit.get("pairs") != shard["pairs"]:
        raise CampaignError("shard audit game/pair counts differ from the manifest")
    pentanomial = audit.get("pentanomial")
    if not isinstance(pentanomial, list) or sum(pentanomial) != shard["pairs"]:
        raise CampaignError("shard audit pentanomial count differs from the manifest")
    wdl = audit.get("wdl")
    if not isinstance(wdl, dict) or sum(
        wdl.get(key, -1) for key in ("wins", "draws", "losses")
    ) != shard["games"]:
        raise CampaignError("shard audit W/D/L count differs from the manifest")

    result = {
        "format": "neyrang-distributed-result-v1",
        "campaign_id": campaign["campaign_id"],
        "campaign_sha256": campaign_sha256,
        "shard_id": shard_id,
        "shard_manifest_sha256": manifest_sha256,
        "index": shard["index"],
        "pair_offset": shard["pair_offset"],
        "pairs": shard["pairs"],
        "games": shard["games"],
        "worker_platform": {
            "os": platform.system(),
            "architecture": platform.machine(),
        },
        "audit_summary": {
            "wdl": wdl,
            "score": audit.get("score"),
            "pentanomial": pentanomial,
        },
        "artifacts": artifacts,
    }
    result_path.write_bytes(pretty_json(result))
    return result


def verify_result_manifest(
    manifest: Path,
    *,
    result_path: Path,
    results: Path,
    campaign_sha256: str,
) -> dict[str, Any]:
    campaign, shard, _, manifest_sha256 = _load_bound_shard(
        manifest, campaign_sha256
    )
    result = json.loads(result_path.read_text(encoding="utf-8"))
    exact = {
        "format": "neyrang-distributed-result-v1",
        "campaign_id": campaign["campaign_id"],
        "campaign_sha256": campaign_sha256,
        "shard_id": shard["shard_id"],
        "shard_manifest_sha256": manifest_sha256,
        "index": shard["index"],
        "pair_offset": shard["pair_offset"],
        "pairs": shard["pairs"],
        "games": shard["games"],
    }
    for field, expected in exact.items():
        if result.get(field) != expected:
            raise CampaignError(
                f"result field {field} is {result.get(field)!r}, expected {expected!r}"
            )
    expected_files = {
        "pgn": f"{shard['shard_id']}.pgn",
        "log": f"{shard['shard_id']}.log",
        "metadata": f"{shard['shard_id']}.meta.txt",
        "config": f"{shard['shard_id']}.config.json",
        "audit": f"{shard['shard_id']}.audit.json",
    }
    artifacts = result.get("artifacts")
    if not isinstance(artifacts, dict) or set(artifacts) != set(expected_files):
        raise CampaignError("result artifact set is incomplete or unexpected")
    for label, filename in expected_files.items():
        recorded = artifacts[label]
        if recorded.get("file") != filename:
            raise CampaignError(f"artifact {label} filename differs from the contract")
        path = results / filename
        actual = sha256_file(path)
        if actual != recorded.get("sha256"):
            raise CampaignError(
                f"artifact {label} SHA-256 is {actual}, expected {recorded.get('sha256')}"
            )
        if path.stat().st_size != recorded.get("bytes"):
            raise CampaignError(f"artifact {label} byte count differs from the contract")
    return result


def audit_campaign_results(
    campaign_path: Path, *, results: Path, campaign_sha256: str
) -> dict[str, Any]:
    actual_campaign_sha256 = sha256_file(campaign_path)
    if actual_campaign_sha256 != campaign_sha256:
        raise CampaignError(
            f"campaign SHA-256 is {actual_campaign_sha256}, expected {campaign_sha256}"
        )
    campaign = json.loads(campaign_path.read_text(encoding="utf-8"))
    if campaign.get("format") != "neyrang-distributed-campaign-v1":
        raise CampaignError(f"unsupported campaign format in {campaign_path}")
    descriptors = sorted(campaign.get("shards", []), key=lambda item: item["index"])
    expected_results = {
        f"{descriptor['shard_id']}.result.json" for descriptor in descriptors
    }
    actual_results = {path.name for path in results.glob("*.result.json")}
    missing = sorted(expected_results - actual_results)
    unexpected = sorted(actual_results - expected_results)
    if missing:
        raise CampaignError(f"missing result manifest(s): {', '.join(missing)}")
    if unexpected:
        raise CampaignError(f"unexpected result manifest(s): {', '.join(unexpected)}")

    wdl = {"wins": 0, "draws": 0, "losses": 0}
    pentanomial = [0, 0, 0, 0, 0]
    games = 0
    pairs = 0
    score = 0.0
    verified_shards: list[dict[str, Any]] = []
    campaign_root = campaign_path.parent
    for descriptor in descriptors:
        manifest = campaign_root / descriptor["manifest_file"]
        result_path = results / f"{descriptor['shard_id']}.result.json"
        result = verify_result_manifest(
            manifest,
            result_path=result_path,
            results=results,
            campaign_sha256=campaign_sha256,
        )
        summary = result["audit_summary"]
        games += result["games"]
        pairs += result["pairs"]
        score += float(summary["score"])
        for key in wdl:
            wdl[key] += int(summary["wdl"][key])
        for index, count in enumerate(summary["pentanomial"]):
            pentanomial[index] += int(count)
        verified_shards.append(
            {
                "shard_id": result["shard_id"],
                "games": result["games"],
                "pairs": result["pairs"],
                "result_sha256": sha256_file(result_path),
            }
        )

    selection = campaign["selection"]
    if games != selection["games"] or pairs != selection["pairs"]:
        raise CampaignError("aggregate game/pair totals differ from campaign.json")
    if sum(wdl.values()) != games or sum(pentanomial) != pairs:
        raise CampaignError("aggregate W/D/L or pentanomial totals are inconsistent")
    return {
        "format": "neyrang-distributed-campaign-audit-v1",
        "ok": True,
        "campaign_id": campaign["campaign_id"],
        "campaign_sha256": campaign_sha256,
        "shards": len(verified_shards),
        "games": games,
        "pairs": pairs,
        "wdl": wdl,
        "score": score,
        "score_percent": 100.0 * score / games if games else 0.0,
        "pentanomial": pentanomial,
        "verified_shards": verified_shards,
    }


def replay_campaign_audits(
    campaign_path: Path,
    *,
    results: Path,
    campaign_sha256: str,
    audit_script: Path,
) -> list[dict[str, Any]]:
    if not audit_script.is_file():
        raise CampaignError(f"audit script is not a file: {audit_script}")
    audit_campaign_results(
        campaign_path, results=results, campaign_sha256=campaign_sha256
    )
    campaign = json.loads(campaign_path.read_text(encoding="utf-8"))
    campaign_root = campaign_path.parent
    replays: list[dict[str, Any]] = []
    for descriptor in sorted(campaign["shards"], key=lambda item: item["index"]):
        manifest = campaign_root / descriptor["manifest_file"]
        shard = json.loads(manifest.read_text(encoding="utf-8"))
        result_path = results / f"{shard['shard_id']}.result.json"
        result = verify_result_manifest(
            manifest,
            result_path=result_path,
            results=results,
            campaign_sha256=campaign_sha256,
        )
        shard_id = shard["shard_id"]
        environment = {
            "PGN_OUT": str(results / f"{shard_id}.pgn"),
            "LOG_OUT": str(results / f"{shard_id}.log"),
            "META_OUT": str(results / f"{shard_id}.meta.txt"),
            "ENGINE_A_NAME": shard["assets"]["engine_a"]["name"],
            "ENGINE_B_NAME": shard["assets"]["engine_b"]["name"],
            "GAMES": str(shard["games"]),
            "TC": shard["match"]["time_control"],
            "OPENINGS_FILE": str(campaign_root / shard["openings_file"]),
            "WARNING_POLICY": shard["match"]["warning_policy"],
            "CAMPAIGN_ID": shard["campaign_id"],
            "SHARD_ID": shard_id,
            "CAMPAIGN_MANIFEST_SHA256": campaign_sha256,
            "SHARD_MANIFEST_SHA256": descriptor["manifest_sha256"],
            "PAIR_OFFSET": str(shard["pair_offset"]),
            "THREADS": str(shard["match"]["threads"]),
            "HASH_MB": str(shard["match"]["hash_mb"]),
            "CONCURRENCY": str(shard["match"]["concurrency"]),
        }
        command = build_audit_command(manifest, environment, audit_script)
        try:
            completed = subprocess.run(
                command, check=True, capture_output=True, text=True
            )
            replay = json.loads(completed.stdout)
        except (subprocess.CalledProcessError, json.JSONDecodeError) as error:
            raise CampaignError(
                f"independent replay failed for {shard_id}: {error}"
            ) from error
        expected = {
            "format": "neyrang-match-audit-v1",
            "ok": True,
            "games": result["games"],
            "pairs": result["pairs"],
            "wdl": result["audit_summary"]["wdl"],
            "score": result["audit_summary"]["score"],
            "pentanomial": result["audit_summary"]["pentanomial"],
        }
        if any(replay.get(field) != value for field, value in expected.items()):
            raise CampaignError(
                f"independent replay differs from retained audit for {shard_id}"
            )
        replays.append(
            {
                "shard_id": shard_id,
                "games": replay["games"],
                "pairs": replay["pairs"],
                "audit_sha256": sha256_bytes(completed.stdout.encode("utf-8")),
            }
        )
    return replays


def _load_bound_shard(
    manifest: Path, campaign_sha256: str
) -> tuple[dict[str, Any], dict[str, Any], dict[str, Any], str]:
    campaign_root = manifest.parent.parent
    campaign_path = campaign_root / "campaign.json"
    actual_campaign_sha256 = sha256_file(campaign_path)
    if actual_campaign_sha256 != campaign_sha256:
        raise CampaignError(
            f"campaign SHA-256 is {actual_campaign_sha256}, expected {campaign_sha256}"
        )
    campaign = json.loads(campaign_path.read_text(encoding="utf-8"))
    shard = json.loads(manifest.read_text(encoding="utf-8"))
    if campaign.get("format") != "neyrang-distributed-campaign-v1":
        raise CampaignError(f"unsupported campaign format in {campaign_path}")
    if shard.get("format") != "neyrang-distributed-shard-v1":
        raise CampaignError(f"unsupported shard format in {manifest}")
    if shard.get("campaign_id") != campaign.get("campaign_id"):
        raise CampaignError("shard campaign_id does not match campaign.json")
    try:
        relative = manifest.relative_to(campaign_root).as_posix()
    except ValueError as error:
        raise CampaignError("shard manifest is outside the campaign root") from error
    descriptor = next(
        (
            item
            for item in campaign.get("shards", [])
            if item.get("manifest_file") == relative
        ),
        None,
    )
    if descriptor is None:
        raise CampaignError(f"campaign does not register {relative}")
    manifest_sha256 = sha256_file(manifest)
    if manifest_sha256 != descriptor.get("manifest_sha256"):
        raise CampaignError(
            f"shard manifest SHA-256 is {manifest_sha256}, "
            f"expected {descriptor.get('manifest_sha256')}"
        )
    for field in (
        "shard_id",
        "index",
        "pair_offset",
        "pairs",
        "games",
        "openings_file",
        "openings_sha256",
    ):
        if shard.get(field) != descriptor.get(field):
            raise CampaignError(f"shard field {field} differs from campaign.json")
    return campaign, shard, descriptor, manifest_sha256


def _validate_config(config: CampaignConfig) -> None:
    if config.output.exists():
        raise CampaignError(f"refusing to overwrite existing output {config.output}")
    for label, path in (
        ("engine_a", config.engine_a),
        ("engine_b", config.engine_b),
        ("fastchess", config.fastchess),
        ("openings", config.openings),
    ):
        if not path.is_file():
            raise CampaignError(f"{label} is not a file: {path}")
    if config.total_pairs < 1:
        raise CampaignError("total_pairs must be positive")
    if config.pairs_per_shard < 1:
        raise CampaignError("pairs_per_shard must be positive")
    if config.hash_mb < 1 or config.threads < 1:
        raise CampaignError("hash_mb and threads must be positive")
    if config.move_overhead_ms < 0 or config.time_margin_ms < 0:
        raise CampaignError("time reserves must be non-negative")
    canonical_time_control(config.time_control)
    if config.warning_policy not in {
        "reject-all",
        "allow-opponent-threefold-pv",
    }:
        raise CampaignError(f"unsupported warning policy {config.warning_policy!r}")
    if config.strict and config.warning_policy != "reject-all":
        raise CampaignError("strict mode requires the reject-all warning policy")


def _read_unique_openings(path: Path) -> list[tuple[str, str]]:
    result: list[tuple[str, str]] = []
    seen: set[str] = set()
    for line_number, raw in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
        line = raw.strip()
        if not line:
            continue
        try:
            fields = line.split()
            if len(fields) == 6:
                board = chess.Board(line)
            else:
                board = chess.Board()
                board.set_epd(line)
        except ValueError as error:
            raise CampaignError(f"invalid EPD at {path}:{line_number}: {error}") from error
        canonical = " ".join(board.fen(en_passant="fen").split()[:4])
        if canonical in seen:
            raise CampaignError(
                f"duplicate canonical opening at {path}:{line_number}: {canonical}"
            )
        seen.add(canonical)
        result.append((canonical, line))
    if not result:
        raise CampaignError(f"opening file is empty: {path}")
    return result


def _asset(path: Path, name: str, git_sha: str) -> dict[str, str]:
    return {
        "filename": path.name,
        "name": name,
        "git_sha": git_sha,
        "sha256": sha256_file(path),
    }


def canonical_time_control(value: str) -> str:
    match = TIME_CONTROL.fullmatch(value)
    if match is None:
        raise CampaignError(f"unsupported time control {value!r}")
    try:
        base = Decimal(match.group("base"))
        increment = (
            Decimal(match.group("increment"))
            if match.group("increment") is not None
            else None
        )
    except InvalidOperation as error:
        raise CampaignError(f"invalid time control {value!r}") from error
    if base <= 0 or increment is not None and increment < 0:
        raise CampaignError("time control values must be positive/non-negative")
    for label, amount in (("base", base), ("increment", increment)):
        if amount is not None and amount * 1000 != (amount * 1000).to_integral_value():
            raise CampaignError(
                f"time-control {label} must have exact millisecond precision"
            )

    def normalized(amount: Decimal) -> str:
        rendered = format(amount, "f")
        return rendered.rstrip("0").rstrip(".") if "." in rendered else rendered

    moves = f"{match.group('moves')}/" if match.group("moves") else ""
    suffix = f"+{normalized(increment)}" if increment is not None else ""
    return f"{moves}{normalized(base)}{suffix}"


def sha256_file(path: Path) -> str:
    if not path.is_file():
        raise CampaignError(f"required asset is not a file: {path}")
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def sha256_bytes(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def canonical_json(data: Any) -> bytes:
    return json.dumps(data, sort_keys=True, separators=(",", ":")).encode("utf-8")


def pretty_json(data: Any) -> bytes:
    return (json.dumps(data, indent=2, sort_keys=True) + "\n").encode("utf-8")


def build_audit_command(
    manifest: Path,
    environment: dict[str, str],
    audit_script: Path,
) -> list[str]:
    shard = json.loads(manifest.read_text(encoding="utf-8"))
    return [
        sys.executable,
        str(audit_script),
        "--pgn",
        environment["PGN_OUT"],
        "--log",
        environment["LOG_OUT"],
        "--meta",
        environment["META_OUT"],
        "--candidate",
        environment["ENGINE_A_NAME"],
        "--opponent",
        environment["ENGINE_B_NAME"],
        "--expected-games",
        environment["GAMES"],
        "--expected-time-control",
        environment["TC"],
        "--expected-openings",
        environment["OPENINGS_FILE"],
        "--warning-policy",
        environment["WARNING_POLICY"],
        "--expect-meta",
        f"campaign_id={environment['CAMPAIGN_ID']}",
        "--expect-meta",
        f"shard_id={environment['SHARD_ID']}",
        "--expect-meta",
        "campaign_manifest_sha256="
        f"{environment['CAMPAIGN_MANIFEST_SHA256']}",
        "--expect-meta",
        f"shard_manifest_sha256={environment['SHARD_MANIFEST_SHA256']}",
        "--expect-meta",
        f"pair_offset={environment['PAIR_OFFSET']}",
        "--expect-meta",
        f"openings_sha256={shard['openings_sha256']}",
        "--expect-meta",
        f"engine_a_sha256={shard['assets']['engine_a']['sha256']}",
        "--expect-meta",
        f"engine_b_sha256={shard['assets']['engine_b']['sha256']}",
        "--expect-meta",
        f"fastchess_sha256={shard['assets']['fastchess']['sha256']}",
        "--expect-meta",
        "opening_order=sequential",
        "--expect-meta",
        f"threads={environment['THREADS']}",
        "--expect-meta",
        f"hash_mb={environment['HASH_MB']}",
        "--expect-meta",
        f"concurrency={environment['CONCURRENCY']}",
    ]


def create_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)

    prepare = commands.add_parser("prepare", help="write immutable campaign shards")
    prepare.add_argument("--engine-a", required=True, type=Path)
    prepare.add_argument("--engine-a-name", required=True)
    prepare.add_argument("--engine-a-git-sha", required=True)
    prepare.add_argument("--engine-b", required=True, type=Path)
    prepare.add_argument("--engine-b-name", required=True)
    prepare.add_argument("--engine-b-git-sha", required=True)
    prepare.add_argument("--fastchess", required=True, type=Path)
    prepare.add_argument("--fastchess-version", required=True)
    prepare.add_argument("--openings", required=True, type=Path)
    prepare.add_argument("--openings-source", required=True)
    prepare.add_argument("--openings-license", required=True)
    prepare.add_argument("--output", required=True, type=Path)
    prepare.add_argument("--seed", required=True)
    prepare.add_argument("--pairs", required=True, type=int)
    prepare.add_argument("--pairs-per-shard", required=True, type=int)
    prepare.add_argument("--tc", required=True)
    prepare.add_argument("--hash-mb", type=int, default=64)
    prepare.add_argument("--threads", type=int, default=1)
    prepare.add_argument("--move-overhead-ms", type=int, default=100)
    prepare.add_argument("--time-margin-ms", type=int, default=0)
    prepare.add_argument("--strict", action="store_true")
    prepare.add_argument(
        "--warning-policy",
        choices=("reject-all", "allow-opponent-threefold-pv"),
        default="reject-all",
    )

    run = commands.add_parser("run-shard", help="verify and execute one shard")
    run.add_argument("--manifest", required=True, type=Path)
    run.add_argument("--campaign-sha256", required=True)
    run.add_argument("--engine-a", required=True, type=Path)
    run.add_argument("--engine-b", required=True, type=Path)
    run.add_argument("--fastchess", required=True, type=Path)
    run.add_argument("--results", required=True, type=Path)
    run.add_argument(
        "--match-script", type=Path, default=Path(__file__).with_name("match.sh")
    )
    run.add_argument(
        "--audit-script",
        type=Path,
        default=Path(__file__).with_name("audit-match.py"),
    )
    run.add_argument("--dry-run", action="store_true")

    audit = commands.add_parser(
        "audit-campaign", help="verify, replay, and aggregate every shard"
    )
    audit.add_argument("--campaign", required=True, type=Path)
    audit.add_argument("--campaign-sha256", required=True)
    audit.add_argument("--results", required=True, type=Path)
    audit.add_argument("--audit-script", required=True, type=Path)
    audit.add_argument("--output", required=True, type=Path)
    return parser


def main() -> int:
    args = create_parser().parse_args()
    try:
        if args.command == "prepare":
            summary = prepare_campaign(
                CampaignConfig(
                    engine_a=args.engine_a,
                    engine_a_name=args.engine_a_name,
                    engine_a_git_sha=args.engine_a_git_sha,
                    engine_b=args.engine_b,
                    engine_b_name=args.engine_b_name,
                    engine_b_git_sha=args.engine_b_git_sha,
                    fastchess=args.fastchess,
                    fastchess_version=args.fastchess_version,
                    openings=args.openings,
                    openings_source=args.openings_source,
                    openings_license=args.openings_license,
                    output=args.output,
                    seed=args.seed,
                    total_pairs=args.pairs,
                    pairs_per_shard=args.pairs_per_shard,
                    time_control=args.tc,
                    hash_mb=args.hash_mb,
                    threads=args.threads,
                    move_overhead_ms=args.move_overhead_ms,
                    time_margin_ms=args.time_margin_ms,
                    strict=args.strict,
                    warning_policy=args.warning_policy,
                )
            )
            print(json.dumps(summary, indent=2, sort_keys=True))
            return 0

        if args.command == "audit-campaign":
            if args.output.exists():
                raise CampaignError(f"refusing to overwrite existing {args.output}")
            summary = audit_campaign_results(
                args.campaign,
                results=args.results,
                campaign_sha256=args.campaign_sha256,
            )
            replays = replay_campaign_audits(
                args.campaign,
                results=args.results,
                campaign_sha256=args.campaign_sha256,
                audit_script=args.audit_script,
            )
            summary["independent_replay"] = True
            summary["replayed_shards"] = len(replays)
            summary["replay_audits"] = replays
            output = pretty_json(summary)
            args.output.parent.mkdir(parents=True, exist_ok=True)
            args.output.write_bytes(output)
            print(output.decode("utf-8"), end="")
            return 0

        environment = build_shard_environment(
            args.manifest,
            engine_a=args.engine_a,
            engine_b=args.engine_b,
            fastchess=args.fastchess,
            results=args.results,
            campaign_sha256=args.campaign_sha256,
        )
        audit_command = build_audit_command(
            args.manifest, environment, args.audit_script
        )
        if args.dry_run:
            print(
                json.dumps(
                    {
                        "status": "verified-dry-run",
                        "match_script": str(args.match_script),
                        "environment": environment,
                        "audit_command": audit_command,
                    },
                    indent=2,
                    sort_keys=True,
                )
            )
            return 0

        if not args.match_script.is_file() or not args.audit_script.is_file():
            raise CampaignError("match and audit scripts must both be files")
        output_paths = [
            Path(environment[key])
            for key in ("PGN_OUT", "LOG_OUT", "META_OUT", "CONFIG_OUT")
        ]
        audit_path = args.results / f"{environment['SHARD_ID']}.audit.json"
        if any(path.exists() for path in [*output_paths, audit_path]):
            raise CampaignError("refusing to overwrite existing shard results")
        args.results.mkdir(parents=True, exist_ok=True)
        process_environment = os.environ.copy()
        process_environment.update(environment)
        subprocess.run(
            [str(args.match_script)], env=process_environment, check=True
        )
        with audit_path.open("w", encoding="utf-8") as output:
            subprocess.run(audit_command, stdout=output, check=True, text=True)
        result = finalize_shard_result(
            args.manifest,
            results=args.results,
            campaign_sha256=args.campaign_sha256,
        )
        print(json.dumps(result, indent=2, sort_keys=True))
        return 0
    except (CampaignError, OSError, subprocess.CalledProcessError) as error:
        print(f"distributed test failed: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
