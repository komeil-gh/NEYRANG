#!/usr/bin/env python3
"""Measure NEYRANG UCI deadline behavior from outside the engine process."""

from __future__ import annotations

import argparse
import csv
import hashlib
import json
import math
import os
import platform
import queue
import random
import re
import statistics
import subprocess
import sys
import threading
import time
from dataclasses import asdict, dataclass
from datetime import datetime, timezone
from pathlib import Path
from typing import Callable


INFO_TIME = re.compile(r"(?:^|\s)time (\d+)(?:\s|$)")


@dataclass(frozen=True)
class Sample:
    requested_ms: int
    sample_index: int
    position_index: int
    engine_reported_ms: int | None
    wall_ms: float
    latency_ms: float | None
    overshoot_ms: float
    bestmove: str


class UciProcess:
    def __init__(self, executable: Path) -> None:
        self.process = subprocess.Popen(
            [str(executable)],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            bufsize=1,
            text=True,
        )
        if self.process.stdin is None or self.process.stdout is None:
            raise RuntimeError("failed to create UCI pipes")
        self.lines: queue.Queue[str | None] = queue.Queue()
        self.reader = threading.Thread(target=self._read_stdout, daemon=True)
        self.reader.start()

    def _read_stdout(self) -> None:
        assert self.process.stdout is not None
        try:
            for line in self.process.stdout:
                self.lines.put(line.rstrip("\r\n"))
        finally:
            self.lines.put(None)

    def send(self, command: str) -> None:
        if self.process.poll() is not None:
            raise RuntimeError(f"engine exited with status {self.process.returncode}")
        assert self.process.stdin is not None
        self.process.stdin.write(f"{command}\n")
        self.process.stdin.flush()

    def wait_until(
        self,
        predicate: Callable[[str], bool],
        timeout_seconds: float,
    ) -> list[str]:
        deadline = time.monotonic() + timeout_seconds
        observed: list[str] = []
        while True:
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                raise TimeoutError(f"UCI response timed out; observed={observed[-8:]}")
            try:
                line = self.lines.get(timeout=remaining)
            except queue.Empty as error:
                raise TimeoutError(
                    f"UCI response timed out; observed={observed[-8:]}"
                ) from error
            if line is None:
                raise RuntimeError(
                    f"engine exited with status {self.process.poll()}; "
                    f"observed={observed[-8:]}"
                )
            observed.append(line)
            if predicate(line):
                return observed

    def initialize(self, hash_mb: int, threads: int, move_overhead_ms: int) -> str:
        self.send("uci")
        identity_lines = self.wait_until(lambda line: line == "uciok", 5.0)
        identity = next(
            (line.removeprefix("id name ") for line in identity_lines if line.startswith("id name ")),
            "unknown",
        )
        self.send(f"setoption name Hash value {hash_mb}")
        self.send(f"setoption name Threads value {threads}")
        self.send(f"setoption name Move Overhead value {move_overhead_ms}")
        self.send("ucinewgame")
        self.send("isready")
        self.wait_until(lambda line: line == "readyok", 10.0)
        return identity

    def measure(self, fen: str, movetime_ms: int, timeout_seconds: float) -> tuple[int | None, float, str]:
        self.send(f"position fen {fen}")
        started_ns = time.perf_counter_ns()
        self.send(f"go movetime {movetime_ms}")
        lines = self.wait_until(lambda line: line.startswith("bestmove "), timeout_seconds)
        wall_ms = (time.perf_counter_ns() - started_ns) / 1_000_000
        reported_times = [
            int(match.group(1))
            for line in lines
            if (match := INFO_TIME.search(line)) is not None
        ]
        bestmove = lines[-1].split(maxsplit=1)[1]
        if bestmove == "0000":
            raise RuntimeError(f"engine returned no legal move for FEN: {fen}")
        return (reported_times[-1] if reported_times else None), wall_ms, bestmove

    def close(self) -> None:
        if self.process.poll() is not None:
            return
        try:
            self.send("quit")
            self.process.wait(timeout=2.0)
        except (BrokenPipeError, RuntimeError, subprocess.TimeoutExpired):
            self.process.terminate()
            try:
                self.process.wait(timeout=2.0)
            except subprocess.TimeoutExpired:
                self.process.kill()
                self.process.wait(timeout=2.0)


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for block in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def percentile(values: list[float], percentile_value: float) -> float:
    if not values:
        raise ValueError("cannot calculate a percentile of an empty sample")
    ordered = sorted(values)
    rank = max(0, math.ceil(percentile_value * len(ordered)) - 1)
    return ordered[rank]


def distribution(values: list[float]) -> dict[str, float]:
    return {
        "median": round(statistics.median(values), 3),
        "p95": round(percentile(values, 0.95), 3),
        "p99": round(percentile(values, 0.99), 3),
        "maximum": round(max(values), 3),
    }


def summarize(samples: list[Sample], movetimes: list[int]) -> dict[str, object]:
    summary: dict[str, object] = {}
    for requested_ms in movetimes:
        group = [sample for sample in samples if sample.requested_ms == requested_ms]
        reported = [
            float(sample.engine_reported_ms)
            for sample in group
            if sample.engine_reported_ms is not None
        ]
        latency = [
            sample.latency_ms for sample in group if sample.latency_ms is not None
        ]
        summary[str(requested_ms)] = {
            "samples": len(group),
            "engine_reported_ms": distribution(reported) if reported else None,
            "wall_ms": distribution([sample.wall_ms for sample in group]),
            "latency_ms": distribution(latency) if latency else None,
            "overshoot_ms": distribution([sample.overshoot_ms for sample in group]),
        }
    return summary


def parse_movetimes(value: str) -> list[int]:
    parsed = [int(item) for item in value.split(",") if item]
    if not parsed or any(item <= 0 for item in parsed) or len(set(parsed)) != len(parsed):
        raise argparse.ArgumentTypeError("movetimes must be unique positive integers")
    return parsed


def write_csv(path: Path, samples: list[Sample]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("w", newline="", encoding="utf-8") as output:
        writer = csv.DictWriter(output, fieldnames=list(asdict(samples[0])))
        writer.writeheader()
        writer.writerows(asdict(sample) for sample in samples)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--engine", required=True, type=Path)
    parser.add_argument("--openings", required=True, type=Path)
    parser.add_argument("--movetimes", type=parse_movetimes, default=parse_movetimes("5,10,20,50,100"))
    parser.add_argument("--samples-per-limit", type=int, default=100)
    parser.add_argument("--move-overhead", type=int, default=10)
    parser.add_argument("--hash", type=int, default=64)
    parser.add_argument("--threads", type=int, default=1)
    parser.add_argument("--seed", type=int, default=20260829)
    parser.add_argument("--sample-timeout", type=float, default=5.0)
    parser.add_argument("--output-json", type=Path)
    parser.add_argument("--output-csv", type=Path)
    args = parser.parse_args()

    if args.samples_per_limit <= 0:
        parser.error("--samples-per-limit must be positive")
    if args.move_overhead < 0 or args.hash <= 0 or args.threads <= 0:
        parser.error("Move Overhead must be non-negative; Hash and Threads must be positive")
    if args.sample_timeout <= 0:
        parser.error("--sample-timeout must be positive")

    engine = args.engine.resolve(strict=True)
    openings = args.openings.resolve(strict=True)
    if not os.access(engine, os.X_OK):
        parser.error(f"engine is not executable: {engine}")
    positions = [line.strip() for line in openings.read_text(encoding="utf-8").splitlines() if line.strip()]
    sample_count = args.samples_per_limit * len(args.movetimes)
    if len(positions) < sample_count:
        parser.error(f"opening file has {len(positions)} positions; {sample_count} are required")

    rng = random.Random(args.seed)
    position_indices = rng.sample(range(len(positions)), sample_count)
    schedule = [movetime for movetime in args.movetimes for _ in range(args.samples_per_limit)]
    rng.shuffle(schedule)

    started_utc = datetime.now(timezone.utc).isoformat()
    samples: list[Sample] = []
    per_limit_index = {movetime: 0 for movetime in args.movetimes}
    uci = UciProcess(engine)
    try:
        identity = uci.initialize(args.hash, args.threads, args.move_overhead)
        for position_index, requested_ms in zip(position_indices, schedule, strict=True):
            per_limit_index[requested_ms] += 1
            engine_ms, wall_ms, bestmove = uci.measure(
                positions[position_index], requested_ms, args.sample_timeout
            )
            latency_ms = None if engine_ms is None else wall_ms - engine_ms
            samples.append(
                Sample(
                    requested_ms=requested_ms,
                    sample_index=per_limit_index[requested_ms],
                    position_index=position_index,
                    engine_reported_ms=engine_ms,
                    wall_ms=round(wall_ms, 3),
                    latency_ms=None if latency_ms is None else round(latency_ms, 3),
                    overshoot_ms=round(wall_ms - requested_ms, 3),
                    bestmove=bestmove,
                )
            )
    finally:
        uci.close()

    summary = summarize(samples, args.movetimes)
    result = {
        "format": "neyrang-timing-audit-v1",
        "started_utc": started_utc,
        "completed_utc": datetime.now(timezone.utc).isoformat(),
        "engine": str(engine),
        "engine_identity": identity,
        "engine_sha256": sha256(engine),
        "openings": str(openings),
        "openings_sha256": sha256(openings),
        "seed": args.seed,
        "movetimes_ms": args.movetimes,
        "samples_per_limit": args.samples_per_limit,
        "move_overhead_ms": args.move_overhead,
        "hash_mb": args.hash,
        "threads": args.threads,
        "clock": "time.perf_counter_ns (monotonic)",
        "host": platform.platform(),
        "python": sys.version.split()[0],
        "summary": summary,
        "samples": [asdict(sample) for sample in samples],
    }

    if args.output_json:
        args.output_json.parent.mkdir(parents=True, exist_ok=True)
        args.output_json.write_text(
            json.dumps(result, indent=2, sort_keys=True) + "\n", encoding="utf-8"
        )
    if args.output_csv:
        write_csv(args.output_csv, samples)

    print("movetime  samples  wall median/p95/p99/max ms  overshoot median/p95/p99/max ms")
    for requested_ms in args.movetimes:
        item = summary[str(requested_ms)]
        wall = item["wall_ms"]
        overshoot = item["overshoot_ms"]
        assert isinstance(wall, dict) and isinstance(overshoot, dict)
        print(
            f"{requested_ms:>8}  {item['samples']:>7}  "
            f"{wall['median']:>7.3f}/{wall['p95']:>7.3f}/{wall['p99']:>7.3f}/{wall['maximum']:>7.3f}  "
            f"{overshoot['median']:>+8.3f}/{overshoot['p95']:>+8.3f}/"
            f"{overshoot['p99']:>+8.3f}/{overshoot['maximum']:>+8.3f}"
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
