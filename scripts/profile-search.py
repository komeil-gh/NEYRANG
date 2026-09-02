#!/usr/bin/env python3
"""Capture and analyze an external macOS sampling profile of NEYRANG search."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import subprocess
import sys
import tempfile
from pathlib import Path
from typing import Any


SCHEMA = "neyrang-search-profile-v1"
BENCH_KEYS = ("positions", "nodes", "time_ms", "nps", "checksum")
CATEGORY_ORDER = (
    "classical_evaluation",
    "exact_see",
    "move_picker",
    "legal_move_generation",
    "make_unmake",
)


class ProfileError(Exception):
    """A fail-closed validation error suitable for concise CLI output."""


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def require_input_file(path: Path, label: str) -> None:
    if not path.is_file():
        raise ProfileError(f"{label} is not a file: {path}")


def require_new_output(path: Path, label: str) -> None:
    if path.exists():
        raise ProfileError(f"refusing to overwrite existing {label}: {path}")
    path.parent.mkdir(parents=True, exist_ok=True)


def parse_benchmark(text: str) -> dict[str, Any]:
    patterns = {
        "positions": re.compile(r"positions:\s*(\d+)"),
        "nodes": re.compile(r"nodes:\s*(\d+)"),
        "time_ms": re.compile(r"time:\s*(\d+)\s+ms"),
        "nps": re.compile(r"nps:\s*(\d+)"),
        "checksum": re.compile(r"checksum:\s*([0-9a-f]{16})"),
    }
    values: dict[str, Any] = {}
    for raw_line in text.splitlines():
        line = raw_line.strip()
        if not line:
            continue
        matched = False
        for key, pattern in patterns.items():
            match = pattern.fullmatch(line)
            if match is None:
                continue
            if key in values:
                raise ProfileError(f"duplicate benchmark field: {key}")
            values[key] = match.group(1) if key == "checksum" else int(match.group(1))
            matched = True
            break
        if not matched:
            raise ProfileError(f"unexpected benchmark output: {line}")

    missing = [key for key in BENCH_KEYS if key not in values]
    if missing:
        raise ProfileError(f"missing benchmark fields: {', '.join(missing)}")
    for key in ("positions", "nodes", "time_ms", "nps"):
        if values[key] <= 0:
            raise ProfileError(f"benchmark field must be positive: {key}")
    return values


def sample_metadata(text: str, label: str) -> str:
    match = re.search(rf"(?m)^{re.escape(label)}:\s*(.+?)\s*$", text)
    if match is None:
        raise ProfileError(f"missing sample metadata: {label}")
    return match.group(1)


def classify_symbol(symbol: str) -> str | None:
    lowered = symbol.lower()
    if "movepicker" in lowered or "shegerd8ordering" in lowered:
        return "move_picker"
    if "shegerd3see" in lowered:
        return "exact_see"
    if "sanj9classical" in lowered or "sanj5pawns" in lowered:
        return "classical_evaluation"
    if "chess7movegen" in lowered:
        return "legal_move_generation"
    if "make_move" in lowered or "unmake_move" in lowered:
        return "make_unmake"
    return None


def parse_sample(text: str) -> tuple[dict[str, str], dict[str, Any]]:
    if "Call graph:" not in text or "Binary Images:" not in text:
        raise ProfileError("sample report is incomplete")

    platform = sample_metadata(text, "Platform")
    analysis_tool = sample_metadata(text, "Analysis Tool")
    if platform != "macOS":
        raise ProfileError(f"unsupported sample platform: {platform}")
    if Path(analysis_tool).name != "sample":
        raise ProfileError(f"unexpected analysis tool: {analysis_tool}")

    heading = re.search(
        r"(?m)^Analysis of sampling .+ every (\d+) millisecond(?:s)?\s*$", text
    )
    if heading is None:
        raise ProfileError("missing sample interval header")

    call_graph = text.split("Call graph:", 1)[1].split("Total number in stack", 1)[0]
    thread_samples = [
        int(match.group(1))
        for match in re.finditer(r"(?m)^\s*(\d+)\s+Thread_", call_graph)
    ]
    if not thread_samples or sum(thread_samples) <= 0:
        raise ProfileError("sample report has no thread sample counts")
    total = sum(thread_samples)

    collapsed_heading = "Sort by top of stack, same collapsed"
    if collapsed_heading not in text:
        raise ProfileError("sample report has no collapsed top-of-stack section")
    collapsed = text.split(collapsed_heading, 1)[1].split("Binary Images:", 1)[0]
    entries: list[tuple[str, int]] = []
    for line in collapsed.splitlines():
        match = re.match(r"^\s*(.+?)\s{2,}(\d+)\s*$", line)
        if match is not None:
            entries.append((match.group(1), int(match.group(2))))
    if not entries:
        raise ProfileError("sample report has no visible collapsed samples")

    visible = sum(count for _, count in entries)
    if visible > total:
        raise ProfileError("visible collapsed samples exceed total samples")

    counts = {name: 0 for name in CATEGORY_ORDER}
    for symbol, count in entries:
        category = classify_symbol(symbol)
        if category is not None:
            counts[category] += count
    classified = sum(counts.values())

    metadata = {
        "process": sample_metadata(text, "Process"),
        "path": sample_metadata(text, "Path"),
        "code_type": sample_metadata(text, "Code Type"),
        "platform": platform,
        "date_time": sample_metadata(text, "Date/Time"),
        "os_version": sample_metadata(text, "OS Version"),
        "analysis_tool": analysis_tool,
        "reported_interval_ms": heading.group(1),
    }
    samples = {
        "total": total,
        "classified": classified,
        "unclassified_visible": visible - classified,
        "suppressed": total - visible,
        "categories": {
            name: {
                "count": counts[name],
                "share_percent": round(100.0 * counts[name] / total, 6),
            }
            for name in CATEGORY_ORDER
        },
    }
    return metadata, samples


def make_report(
    *,
    engine: Path,
    engine_hash: str,
    unchanged: bool,
    bench_text: str,
    profile_path: Path,
    profile_text: str,
    depth: int,
    duration: int,
    interval_ms: int,
    command: list[str],
) -> dict[str, Any]:
    benchmark = parse_benchmark(bench_text)
    metadata, samples = parse_sample(profile_text)
    benchmark["stdout_sha256"] = hashlib.sha256(bench_text.encode("utf-8")).hexdigest()
    return {
        "schema": SCHEMA,
        "engine": {
            "path": str(engine),
            "sha256": engine_hash,
            "unchanged_during_capture": unchanged,
        },
        "workload": {"command": command, "depth": depth},
        "sampler": {"duration_seconds": duration, "interval_ms": interval_ms},
        "benchmark": benchmark,
        "profile": {
            "path": str(profile_path),
            "raw_sha256": sha256_file(profile_path),
            "metadata": metadata,
        },
        "samples": samples,
    }


def write_json_exclusive(path: Path, payload: dict[str, Any]) -> None:
    try:
        with path.open("x", encoding="utf-8") as destination:
            json.dump(payload, destination, indent=2, sort_keys=True)
            destination.write("\n")
    except FileExistsError as error:
        raise ProfileError(f"refusing to overwrite existing JSON report: {path}") from error


def stop_process(process: subprocess.Popen[str]) -> tuple[str, str]:
    if process.poll() is None:
        process.terminate()
        try:
            return process.communicate(timeout=1)
        except subprocess.TimeoutExpired:
            process.kill()
    return process.communicate()


def run_analyze(args: argparse.Namespace) -> None:
    engine = Path(args.engine)
    bench_path = Path(args.bench_output)
    profile_path = Path(args.profile)
    output_path = Path(args.output_json)
    require_input_file(engine, "engine")
    require_input_file(bench_path, "benchmark output")
    require_input_file(profile_path, "sample profile")
    require_new_output(output_path, "JSON report")

    report = make_report(
        engine=engine,
        engine_hash=sha256_file(engine),
        unchanged=True,
        bench_text=bench_path.read_text(encoding="utf-8"),
        profile_path=profile_path,
        profile_text=profile_path.read_text(encoding="utf-8"),
        depth=args.depth,
        duration=args.duration,
        interval_ms=args.interval_ms,
        command=[str(engine), "bench", str(args.depth)],
    )
    write_json_exclusive(output_path, report)


def run_capture(args: argparse.Namespace) -> None:
    engine = Path(args.engine)
    sample_tool = Path(args.sample_tool)
    raw_path = Path(args.raw_profile)
    output_path = Path(args.output_json)
    require_input_file(engine, "engine")
    require_input_file(sample_tool, "sample tool")
    if not os.access(engine, os.X_OK):
        raise ProfileError(f"engine is not executable: {engine}")
    if not os.access(sample_tool, os.X_OK):
        raise ProfileError(f"sample tool is not executable: {sample_tool}")
    require_new_output(raw_path, "raw profile")
    require_new_output(output_path, "JSON report")

    before_hash = sha256_file(engine)
    command = [str(engine), "bench", str(args.depth)]
    temporary = tempfile.NamedTemporaryFile(
        prefix=f".{raw_path.name}.", suffix=".tmp", dir=raw_path.parent, delete=False
    )
    temporary_path = Path(temporary.name)
    temporary.close()

    engine_process: subprocess.Popen[str] | None = None
    raw_published = False
    try:
        engine_process = subprocess.Popen(
            command,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
        )
        sampler_command = [
            str(sample_tool),
            str(engine_process.pid),
            str(args.duration),
            str(args.interval_ms),
            "-mayDie",
            "-file",
            str(temporary_path),
        ]
        try:
            sampled = subprocess.run(
                sampler_command,
                text=True,
                capture_output=True,
                timeout=args.duration + 30,
                check=False,
            )
        except subprocess.TimeoutExpired as error:
            stop_process(engine_process)
            raise ProfileError("sample tool timed out") from error
        if sampled.returncode != 0:
            stop_process(engine_process)
            detail = (sampled.stderr or sampled.stdout).strip()
            suffix = f": {detail}" if detail else ""
            raise ProfileError(f"sample tool exited {sampled.returncode}{suffix}")

        try:
            stdout, stderr = engine_process.communicate(timeout=args.timeout)
        except subprocess.TimeoutExpired as error:
            stop_process(engine_process)
            raise ProfileError("engine benchmark timed out") from error
        if engine_process.returncode != 0:
            detail = stderr.strip()
            suffix = f": {detail}" if detail else ""
            raise ProfileError(f"engine benchmark exited {engine_process.returncode}{suffix}")
        if not temporary_path.is_file() or temporary_path.stat().st_size == 0:
            raise ProfileError("sample tool did not produce a profile")

        after_hash = sha256_file(engine)
        if before_hash != after_hash:
            raise ProfileError("engine binary changed during capture")
        profile_text = temporary_path.read_text(encoding="utf-8")
        report = make_report(
            engine=engine,
            engine_hash=before_hash,
            unchanged=True,
            bench_text=stdout,
            profile_path=temporary_path,
            profile_text=profile_text,
            depth=args.depth,
            duration=args.duration,
            interval_ms=args.interval_ms,
            command=command,
        )

        try:
            with raw_path.open("xb") as destination:
                destination.write(temporary_path.read_bytes())
            raw_published = True
            report["profile"]["path"] = str(raw_path)
            write_json_exclusive(output_path, report)
        except Exception:
            if raw_published:
                raw_path.unlink(missing_ok=True)
            raise
    finally:
        if engine_process is not None and engine_process.poll() is None:
            stop_process(engine_process)
        temporary_path.unlink(missing_ok=True)


def positive_int(value: str) -> int:
    parsed = int(value)
    if parsed <= 0:
        raise argparse.ArgumentTypeError("must be a positive integer")
    return parsed


def depth_value(value: str) -> int:
    parsed = positive_int(value)
    if parsed > 127:
        raise argparse.ArgumentTypeError("must be at most 127")
    return parsed


def add_common_arguments(parser: argparse.ArgumentParser) -> None:
    parser.add_argument("--engine", required=True)
    parser.add_argument("--depth", required=True, type=depth_value)
    parser.add_argument("--duration", required=True, type=positive_int)
    parser.add_argument("--interval-ms", required=True, type=positive_int)
    parser.add_argument("--output-json", required=True)


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Capture or analyze a hash-bound external NEYRANG search profile."
    )
    subcommands = parser.add_subparsers(dest="command", required=True)

    analyze = subcommands.add_parser("analyze", help="analyze existing benchmark/sample files")
    add_common_arguments(analyze)
    analyze.add_argument("--bench-output", required=True)
    analyze.add_argument("--profile", required=True)
    analyze.set_defaults(handler=run_analyze)

    capture = subcommands.add_parser("capture", help="run a benchmark under macOS sample")
    add_common_arguments(capture)
    capture.add_argument("--sample-tool", default="/usr/bin/sample")
    capture.add_argument("--timeout", required=True, type=positive_int)
    capture.add_argument("--raw-profile", required=True)
    capture.set_defaults(handler=run_capture)
    return parser


def main() -> int:
    parser = build_parser()
    args = parser.parse_args()
    try:
        args.handler(args)
    except (ProfileError, OSError, UnicodeError, ValueError) as error:
        print(f"profile-search: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
