#!/usr/bin/env python3
"""Measure NEYRANG's registered 1/2/4-thread UCI scaling and deadline gates."""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import os
import platform
import selectors
import statistics
import subprocess
import sys
import time
from pathlib import Path
from typing import Callable

import chess


SCHEMA = "neyrang-smp-scaling-v1"
THREAD_THRESHOLDS = {2: 1.35, 4: 1.70}
MAX_P95_OVERSHOOT_MS = 30.0
POSITIONS = [
    chess.STARTING_FEN,
    "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1",
    "8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1",
    "rnbq1k1r/pp1Pbppp/2p5/8/2B5/8/PPP1NnPP/RNBQK2R w KQ - 1 8",
    "r4rk1/1pp1qppp/p1np1n2/2b1p1B1/2B1P1b1/P1NP1N2/1PP1QPPP/R4RK1 w - - 0 10",
]


def rotating_schedule(threads: list[int], runs: int) -> list[list[int]]:
    if not threads:
        raise ValueError("at least one thread configuration is required")
    return [threads[index % len(threads) :] + threads[: index % len(threads)] for index in range(runs)]


def parse_info(line: str) -> dict[str, object]:
    tokens = line.split()
    if not tokens or tokens[0] != "info":
        raise ValueError(f"not a UCI info line: {line}")

    numeric: dict[str, int] = {}
    score: dict[str, object] | None = None
    pv: list[str] | None = None
    index = 1
    while index < len(tokens):
        token = tokens[index]
        if token in {"depth", "seldepth", "nodes", "nps", "hashfull", "time"}:
            if index + 1 >= len(tokens):
                raise ValueError(f"missing value after info field: {token}")
            try:
                numeric[token] = int(tokens[index + 1])
            except ValueError as error:
                raise ValueError(f"non-integer info field {token}: {tokens[index + 1]}") from error
            index += 2
        elif token == "score":
            if index + 2 >= len(tokens) or tokens[index + 1] not in {"cp", "mate"}:
                raise ValueError(f"malformed score field: {line}")
            try:
                value = int(tokens[index + 2])
            except ValueError as error:
                raise ValueError(f"non-integer score: {tokens[index + 2]}") from error
            score = {"kind": tokens[index + 1], "value": value}
            index += 3
        elif token == "pv":
            pv = tokens[index + 1 :]
            break
        else:
            index += 1

    for field in ["depth", "seldepth", "nodes", "nps", "hashfull", "time"]:
        if field not in numeric:
            raise ValueError(f"missing info field: {field}")
    if score is None:
        raise ValueError("missing info field: score")
    if pv is None:
        raise ValueError("missing info field: pv")
    return {**numeric, "score": score, "pv": pv}


def nearest_rank(values: list[float | int], percentile: float) -> float | int:
    if not values:
        raise ValueError("percentile requires at least one value")
    if not 0 < percentile <= 1:
        raise ValueError("percentile must be in (0, 1]")
    ordered = sorted(values)
    return ordered[max(0, math.ceil(percentile * len(ordered)) - 1)]


def summarize(
    samples: list[dict[str, object]], threads: list[int]
) -> tuple[dict[str, dict[str, object]], dict[str, object]]:
    grouped: dict[int, list[dict[str, object]]] = {
        value: [sample for sample in samples if sample["threads"] == value]
        for value in threads
    }
    if 1 not in grouped or not grouped[1]:
        raise ValueError("Threads 1 samples are required for scaling")

    baseline_nps = statistics.median(
        int(sample["aggregate_nps"]) for sample in grouped[1]
    )
    summary: dict[str, dict[str, object]] = {}
    for value in threads:
        current = grouped[value]
        if not current:
            raise ValueError(f"no samples for Threads {value}")
        nps_values = [int(sample["aggregate_nps"]) for sample in current]
        overshoots = [
            float(position["deadline_overshoot_ms"])
            for sample in current
            for position in sample["positions"]
        ]
        depths = [
            int(position["info"]["depth"])
            for sample in current
            for position in sample["positions"]
            if "info" in position
        ]
        median_nps = statistics.median(nps_values)
        summary[str(value)] = {
            "runs": len(current),
            "median_aggregate_nps": median_nps,
            "scaling_vs_threads_1": round(median_nps / baseline_nps, 6),
            "p50_deadline_overshoot_ms": nearest_rank(overshoots, 0.5),
            "p95_deadline_overshoot_ms": nearest_rank(overshoots, 0.95),
            "median_completed_depth": statistics.median(depths) if depths else None,
        }

    failures: list[str] = []
    for value in threads:
        p95 = float(summary[str(value)]["p95_deadline_overshoot_ms"])
        if p95 > MAX_P95_OVERSHOOT_MS:
            failures.append(
                f"Threads {value} p95 deadline overshoot {p95:.3f} ms exceeds {MAX_P95_OVERSHOOT_MS:.3f} ms"
            )
    for value, threshold in THREAD_THRESHOLDS.items():
        if value not in grouped:
            failures.append(f"registered Threads {value} configuration is missing")
            continue
        scaling = float(summary[str(value)]["scaling_vs_threads_1"])
        if scaling < threshold:
            failures.append(
                f"Threads {value} scaling {scaling:.6f}x is below {threshold:.2f}x"
            )
    return summary, {"passed": not failures, "failures": failures}


class UciProcess:
    def __init__(
        self,
        engine: Path,
        threads: int,
        hash_megabytes: int,
        move_overhead_ms: int,
        timeout_seconds: float,
    ) -> None:
        self.timeout_seconds = timeout_seconds
        self.move_overhead_ms = move_overhead_ms
        self.process = subprocess.Popen(
            [str(engine)],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            bufsize=0,
        )
        if self.process.stdin is None or self.process.stdout is None:
            raise RuntimeError("failed to open UCI pipes")
        self.stdin = self.process.stdin
        self.stdout = self.process.stdout
        self.stdout_buffer = b""
        self.selector = selectors.DefaultSelector()
        self.selector.register(self.stdout, selectors.EVENT_READ)
        try:
            self.send("uci")
            self.read_until(lambda line: line == "uciok")
            self.send(f"setoption name Hash value {hash_megabytes}")
            self.send(f"setoption name Threads value {threads}")
            self.send(f"setoption name Move Overhead value {move_overhead_ms}")
            self.ready()
        except BaseException:
            self.close()
            raise

    def send(self, command: str) -> None:
        self.stdin.write(f"{command}\n".encode())
        self.stdin.flush()

    def read_until(self, predicate: Callable[[str], bool]) -> list[str]:
        deadline = time.monotonic() + self.timeout_seconds
        lines: list[str] = []
        while True:
            line = self.read_line(deadline)
            lines.append(line)
            if predicate(line):
                return lines

    def read_line(self, deadline: float) -> str:
        while b"\n" not in self.stdout_buffer:
            remaining = deadline - time.monotonic()
            if remaining <= 0 or not self.selector.select(max(0.0, remaining)):
                raise TimeoutError(
                    f"UCI response exceeded {self.timeout_seconds:.3f} seconds"
                )
            chunk = os.read(self.stdout.fileno(), 65_536)
            if not chunk:
                raise RuntimeError(
                    f"engine exited before completing UCI response (status={self.process.poll()})"
                )
            self.stdout_buffer += chunk
        raw, self.stdout_buffer = self.stdout_buffer.split(b"\n", 1)
        return raw.rstrip(b"\r").decode("utf-8")

    def ready(self) -> None:
        self.send("isready")
        lines = self.read_until(lambda line: line == "readyok")
        if any(line.startswith("bestmove ") for line in lines):
            raise RuntimeError(f"duplicate or late bestmove before readyok: {lines}")

    def search(self, fen: str, movetime_ms: int) -> dict[str, object]:
        self.send("ucinewgame")
        self.ready()
        self.send(f"position fen {fen}")
        started_ns = time.monotonic_ns()
        self.send(f"go movetime {movetime_ms}")
        lines = self.read_until(lambda line: line.startswith("bestmove "))
        elapsed_ns = time.monotonic_ns() - started_ns
        bestmoves = [line for line in lines if line.startswith("bestmove ")]
        if len(bestmoves) != 1:
            raise RuntimeError(f"expected exactly one bestmove: {lines}")
        info_lines = [line for line in lines if line.startswith("info depth ")]
        if not info_lines:
            raise RuntimeError(f"search produced no parseable info line: {lines}")
        info = parse_info(info_lines[-1])
        bestmove = bestmoves[0].split(maxsplit=1)[1]
        board = chess.Board(fen)
        try:
            move = chess.Move.from_uci(bestmove)
        except ValueError as error:
            raise RuntimeError(f"malformed bestmove {bestmove!r}") from error
        if move not in board.legal_moves:
            raise RuntimeError(f"illegal bestmove {bestmove!r} for {fen}")
        wall_ms = elapsed_ns / 1_000_000
        hard_limit_ms = max(0, movetime_ms - self.move_overhead_ms)
        return {
            "fen": fen,
            "bestmove": bestmove,
            "wall_time_ms": round(wall_ms, 6),
            "hard_limit_ms": hard_limit_ms,
            "deadline_overshoot_ms": round(max(0.0, wall_ms - hard_limit_ms), 6),
            "info": info,
            "info_line_count": len(info_lines),
        }

    def close(self) -> None:
        try:
            self.selector.close()
        except Exception:
            pass
        if self.process.poll() is None:
            try:
                self.send("quit")
                self.process.wait(timeout=2)
            except (BrokenPipeError, OSError, subprocess.TimeoutExpired):
                self.process.terminate()
                try:
                    self.process.wait(timeout=2)
                except subprocess.TimeoutExpired:
                    self.process.kill()
                    self.process.wait(timeout=2)
        if self.process.stderr is not None:
            stderr = self.process.stderr.read().decode("utf-8", errors="replace")
            if self.process.returncode not in {None, 0}:
                raise RuntimeError(
                    f"engine exited with {self.process.returncode}: {stderr.strip()}"
                )

    def __enter__(self) -> "UciProcess":
        return self

    def __exit__(self, exc_type: object, exc: object, traceback: object) -> None:
        self.close()


def run_sample(
    engine: Path,
    threads: int,
    run: int,
    excluded_warmup: bool,
    movetime_ms: int,
    hash_megabytes: int,
    move_overhead_ms: int,
) -> dict[str, object]:
    timeout_seconds = max(5.0, movetime_ms / 1_000 + 4.0)
    with UciProcess(
        engine,
        threads,
        hash_megabytes,
        move_overhead_ms,
        timeout_seconds,
    ) as process:
        positions = [process.search(fen, movetime_ms) for fen in POSITIONS]
    total_nodes = sum(int(position["info"]["nodes"]) for position in positions)
    total_reported_time_ms = sum(int(position["info"]["time"]) for position in positions)
    total_wall_ns = sum(float(position["wall_time_ms"]) for position in positions) * 1_000_000
    return {
        "threads": threads,
        "run": run,
        "excluded_warmup": excluded_warmup,
        "total_nodes": total_nodes,
        "total_reported_time_ms": total_reported_time_ms,
        "total_wall_time_ms": round(total_wall_ns / 1_000_000, 6),
        "aggregate_nps": int(total_nodes * 1_000 / max(1, total_reported_time_ms)),
        "wall_nps": int(total_nodes * 1_000_000_000 / max(1, total_wall_ns)),
        "positions": positions,
    }


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def git_head(root: Path) -> str:
    completed = subprocess.run(
        ["git", "rev-parse", "HEAD"],
        cwd=root,
        check=True,
        capture_output=True,
        text=True,
    )
    return completed.stdout.strip()


def parse_threads(value: str) -> list[int]:
    try:
        threads = [int(item) for item in value.split(",")]
    except ValueError as error:
        raise argparse.ArgumentTypeError("threads must be comma-separated integers") from error
    if len(set(threads)) != len(threads) or any(item < 1 for item in threads):
        raise argparse.ArgumentTypeError("threads must be unique positive integers")
    if threads != [1, 2, 4]:
        raise argparse.ArgumentTypeError("the registered P3 schedule is exactly 1,2,4")
    return threads


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--engine", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--runs", type=int, default=7)
    parser.add_argument("--movetime-ms", type=int, default=1_000)
    parser.add_argument("--hash", dest="hash_megabytes", type=int, default=64)
    parser.add_argument("--move-overhead", dest="move_overhead_ms", type=int, default=30)
    parser.add_argument("--threads", type=parse_threads, default=[1, 2, 4])
    args = parser.parse_args()

    engine = args.engine.resolve()
    output = args.output.resolve()
    if not engine.is_file() or not os.access(engine, os.X_OK):
        parser.error(f"engine is not executable: {engine}")
    if output.exists():
        parser.error(f"refusing to overwrite output: {output}")
    if args.runs != 7 or args.movetime_ms != 1_000:
        parser.error("the registered P3 gate requires exactly 7 runs at 1000 ms")
    if args.hash_megabytes != 64 or args.move_overhead_ms != 30:
        parser.error("the registered P3 gate requires Hash 64 and Move Overhead 30")

    root = Path(__file__).resolve().parents[1]
    warmups = [
        run_sample(
            engine,
            threads,
            -1,
            True,
            args.movetime_ms,
            args.hash_megabytes,
            args.move_overhead_ms,
        )
        for threads in args.threads
    ]
    samples: list[dict[str, object]] = []
    for run, order in enumerate(rotating_schedule(args.threads, args.runs)):
        for threads in order:
            sample = run_sample(
                engine,
                threads,
                run,
                False,
                args.movetime_ms,
                args.hash_megabytes,
                args.move_overhead_ms,
            )
            samples.append(sample)
            print(
                f"run={run + 1}/{args.runs} threads={threads} "
                f"nps={sample['aggregate_nps']} nodes={sample['total_nodes']}",
                file=sys.stderr,
                flush=True,
            )

    summary, decision = summarize(samples, args.threads)
    evidence = {
        "schema": SCHEMA,
        "created_unix_ns": time.time_ns(),
        "engine": {
            "path": str(engine),
            "sha256": sha256_file(engine),
            "bytes": engine.stat().st_size,
            "git_head": git_head(root),
        },
        "host": {
            "platform": platform.platform(),
            "machine": platform.machine(),
            "python": platform.python_version(),
        },
        "configuration": {
            "threads": args.threads,
            "runs": args.runs,
            "excluded_warmups_per_configuration": 1,
            "movetime_ms": args.movetime_ms,
            "hash_megabytes_total": args.hash_megabytes,
            "move_overhead_ms": args.move_overhead_ms,
            "positions": POSITIONS,
            "order": "rotating 1/2/4",
            "aggregate_nps_denominator": "sum of final UCI info time values; Threads 1 covers its last completed iteration and SMP final info covers exact aggregate work",
            "wall_nps_denominator": "external monotonic wall time from go write through bestmove line; retained as diagnostic only",
            "deadline_overshoot_origin": "external wall time minus movetime after configured Move Overhead",
            "percentile": "nearest-rank",
        },
        "registered_thresholds": {
            "scaling": {str(key): value for key, value in THREAD_THRESHOLDS.items()},
            "maximum_p95_deadline_overshoot_ms": MAX_P95_OVERSHOOT_MS,
        },
        "warmups": warmups,
        "samples": samples,
        "summary": summary,
        "decision": decision,
    }
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(
        json.dumps(evidence, ensure_ascii=False, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    print(json.dumps({"output": str(output), "summary": summary, "decision": decision}, indent=2))
    return 0 if decision["passed"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
