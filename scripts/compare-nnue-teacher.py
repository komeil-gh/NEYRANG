#!/usr/bin/env python3
"""Compare two NEYRANG networks with one UCI teacher on a frozen FEN set."""

from __future__ import annotations

import argparse
import hashlib
import json
import math
from pathlib import Path
import queue
import random
import subprocess
import sys
import threading

try:
    import chess
    import chess.engine
except ModuleNotFoundError:
    chess = None


class ComparisonError(RuntimeError):
    pass


def sha256(path: Path) -> str:
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def logistic_cp(score: int) -> float:
    exponent = max(-50.0, min(50.0, -score / 400.0))
    return 1.0 / (1.0 + math.exp(exponent))


def percentile(values: list[float], percentage: float) -> float:
    ordered = sorted(values)
    rank = (len(ordered) - 1) * percentage / 100.0
    lower, upper = math.floor(rank), math.ceil(rank)
    if lower == upper:
        return ordered[lower]
    return ordered[lower] * (upper - rank) + ordered[upper] * (rank - lower)


def paired_summary(
    baseline_errors: list[float],
    candidate_errors: list[float],
    seed: str,
    replicates: int,
) -> dict[str, float | bool]:
    if not baseline_errors or len(baseline_errors) != len(candidate_errors):
        raise ComparisonError("paired errors must be non-empty and equal length")
    if not seed or replicates <= 0:
        raise ComparisonError("bootstrap seed and positive replicate count are required")
    deltas = [candidate - baseline for baseline, candidate in zip(baseline_errors, candidate_errors)]
    generator = random.Random(int.from_bytes(hashlib.sha256(seed.encode()).digest()[:8]))
    count = len(deltas)
    bootstrap = [
        sum(deltas[generator.randrange(count)] for _ in range(count)) / count
        for _ in range(replicates)
    ]
    baseline_mse = sum(baseline_errors) / count
    candidate_mse = sum(candidate_errors) / count
    upper = percentile(bootstrap, 97.5)
    return {
        "baseline_mse": baseline_mse,
        "candidate_mse": candidate_mse,
        "relative_improvement": 1.0 - candidate_mse / baseline_mse,
        "paired_delta_mean": candidate_mse - baseline_mse,
        "paired_delta_lower_95": percentile(bootstrap, 2.5),
        "paired_delta_upper_95": upper,
        "passed": candidate_mse < baseline_mse and upper < 0.0,
    }


class NeyrangEval:
    def __init__(self, engine: Path, network: Path):
        self.process = subprocess.Popen(
            [str(engine)],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            bufsize=1,
        )
        self.lines: queue.Queue[str | None] = queue.Queue()
        threading.Thread(target=self._read, daemon=True).start()
        self._send("uci")
        if not any("option name EvalFile" in line for line in self._until("uciok")):
            raise ComparisonError("NEYRANG engine lacks EvalFile")
        self._send(f"setoption name EvalFile value {network}")
        self._send("isready")
        self._until("readyok")

    def _read(self) -> None:
        assert self.process.stdout is not None
        for line in self.process.stdout:
            self.lines.put(line.rstrip("\n"))
        self.lines.put(None)

    def _send(self, command: str) -> None:
        if self.process.poll() is not None or self.process.stdin is None:
            raise ComparisonError("NEYRANG engine exited unexpectedly")
        self.process.stdin.write(command + "\n")
        self.process.stdin.flush()

    def _until(self, marker: str) -> list[str]:
        lines = []
        while True:
            try:
                line = self.lines.get(timeout=10)
            except queue.Empty as error:
                raise ComparisonError(f"NEYRANG timed out waiting for {marker}") from error
            if line is None:
                raise ComparisonError(f"NEYRANG closed output before {marker}")
            lines.append(line)
            if line == marker:
                return lines

    def evaluate(self, fen: str) -> int:
        self._send(f"position fen {fen}")
        self._send("eval")
        while True:
            try:
                line = self.lines.get(timeout=10)
            except queue.Empty as error:
                raise ComparisonError("NEYRANG timed out during eval") from error
            if line is None:
                raise ComparisonError("NEYRANG closed output during eval")
            if line.startswith("info string error:"):
                raise ComparisonError(line)
            if line.startswith("info string eval ") and line.endswith(" cp"):
                return int(line.split()[3])

    def close(self) -> None:
        if self.process.poll() is None:
            self._send("quit")
        try:
            _, stderr = self.process.communicate(timeout=10)
        except subprocess.TimeoutExpired as error:
            self.process.kill()
            self.process.communicate()
            raise ComparisonError("NEYRANG did not exit after quit") from error
        if self.process.returncode != 0 or stderr:
            raise ComparisonError("NEYRANG exited uncleanly")


def read_fens(path: Path) -> list[str]:
    if chess is None:
        raise ComparisonError("python-chess is required")
    fens = []
    seen = set()
    for number, raw in enumerate(path.read_text().splitlines(), 1):
        line = raw.strip()
        if not line or line.startswith("#"):
            continue
        try:
            board = chess.Board(line)
        except ValueError as error:
            raise ComparisonError(f"invalid FEN at line {number}: {error}") from error
        fen = board.fen()
        key = " ".join(fen.split()[:4])
        if key in seen:
            raise ComparisonError(f"duplicate FEN at line {number}")
        seen.add(key)
        fens.append(fen)
    if not fens:
        raise ComparisonError("FEN set is empty")
    return fens


def run(args: argparse.Namespace) -> dict:
    if chess is None:
        raise ComparisonError("python-chess is required")
    paths = (args.teacher, args.engine, args.baseline_network, args.candidate_network, args.fens)
    if not all(path.is_file() for path in paths):
        raise ComparisonError("every engine, network, and FEN path must be a file")
    if args.output.exists():
        raise ComparisonError("refusing to overwrite output")
    if args.nodes <= 0:
        raise ComparisonError("teacher node limit must be positive")

    fens = read_fens(args.fens)
    baseline = None
    candidate = None
    rows = []
    try:
        baseline = NeyrangEval(args.engine, args.baseline_network)
        candidate = NeyrangEval(args.engine, args.candidate_network)
        with chess.engine.SimpleEngine.popen_uci(str(args.teacher), timeout=30) as teacher:
            teacher.configure({"Threads": 1, "Hash": 64, "UCI_ShowWDL": True})
            for fen in fens:
                board = chess.Board(fen)
                info = teacher.analyse(board, chess.engine.Limit(nodes=args.nodes))
                wdl = info["wdl"].pov(chess.WHITE)
                target = (wdl.wins + 0.5 * wdl.draws) / 1000.0
                baseline_cp = baseline.evaluate(fen)
                candidate_cp = candidate.evaluate(fen)
                if board.turn == chess.BLACK:
                    baseline_cp = -baseline_cp
                    candidate_cp = -candidate_cp
                baseline_error = (logistic_cp(baseline_cp) - target) ** 2
                candidate_error = (logistic_cp(candidate_cp) - target) ** 2
                rows.append(
                    {
                        "fen": fen,
                        "teacher_wdl_white": [wdl.wins, wdl.draws, wdl.losses],
                        "baseline_cp_white": baseline_cp,
                        "candidate_cp_white": candidate_cp,
                        "baseline_squared_error": baseline_error,
                        "candidate_squared_error": candidate_error,
                    }
                )
    finally:
        if candidate is not None:
            candidate.close()
        if baseline is not None:
            baseline.close()

    summary = paired_summary(
        [row["baseline_squared_error"] for row in rows],
        [row["candidate_squared_error"] for row in rows],
        args.bootstrap_seed,
        args.bootstrap_replicates,
    )
    report = {
        "schema": "neyrang-nnue-teacher-comparison-v1",
        "positions": len(rows),
        "teacher_nodes": args.nodes,
        "bootstrap_seed": args.bootstrap_seed,
        "bootstrap_replicates": args.bootstrap_replicates,
        "sha256": {
            "teacher": sha256(args.teacher),
            "engine": sha256(args.engine),
            "baseline_network": sha256(args.baseline_network),
            "candidate_network": sha256(args.candidate_network),
            "fens": sha256(args.fens),
        },
        **summary,
        "rows": rows,
    }
    with args.output.open("x") as stream:
        json.dump(report, stream, indent=2, sort_keys=True)
        stream.write("\n")
    return report


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--teacher", required=True, type=Path)
    parser.add_argument("--engine", required=True, type=Path)
    parser.add_argument("--baseline-network", required=True, type=Path)
    parser.add_argument("--candidate-network", required=True, type=Path)
    parser.add_argument("--fens", required=True, type=Path)
    parser.add_argument("--nodes", required=True, type=int)
    parser.add_argument("--bootstrap-seed", required=True)
    parser.add_argument("--bootstrap-replicates", type=int, default=10_000)
    parser.add_argument("--output", required=True, type=Path)
    return parser.parse_args()


def main() -> int:
    try:
        report = run(parse_args())
    except (ComparisonError, OSError, KeyError) as error:
        print(f"compare-nnue-teacher: {error}", file=sys.stderr)
        return 1
    print(json.dumps({key: value for key, value in report.items() if key != "rows"}, sort_keys=True))
    return 0 if report["passed"] else 2


if __name__ == "__main__":
    raise SystemExit(main())
