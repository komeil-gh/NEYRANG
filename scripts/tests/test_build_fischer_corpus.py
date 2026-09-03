import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts" / "build-fischer-corpus.py"


class FischerCorpusBuilderTests(unittest.TestCase):
    def run_builder(self, source: Path, output: Path, *extra: str) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            [
                sys.executable,
                str(SCRIPT),
                "--source",
                f"fixture={source}",
                "--source-url",
                "fixture=https://example.test/fischer.pgn",
                "--output-dir",
                str(output),
                "--seed",
                "7",
                "--min-ply",
                "2",
                *extra,
            ],
            cwd=ROOT,
            text=True,
            capture_output=True,
        )

    def test_build_is_deterministic_annotation_free_and_deduplicated(self) -> None:
        pgn = """[Event "A"]
[Site "X"]
[Date "1970.01.01"]
[Round "1"]
[White "Fischer, Robert James"]
[Black "Opponent"]
[Result "1-0"]

1. e4 {remove me} e5 2. Nf3 Nc6 3. Bb5 a6 1-0

[Event "duplicate"]
[Site "Y"]
[Date "1970.01.02"]
[Round "2"]
[White "Bobby Fischer"]
[Black "Opponent"]
[Result "1-0"]

1. e4 e5 2. Nf3 Nc6 3. Bb5 a6 1-0

[Event "B"]
[Site "X"]
[Date "1971.01.01"]
[Round "3"]
[White "Opponent"]
[Black "Robert James Fischer"]
[Result "0-1"]

1. d4 Nf6 2. c4 g6 3. Nc3 Bg7 0-1
"""
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / "games.pgn"
            source.write_text(pgn)
            first = root / "first"
            second = root / "second"
            self.assertEqual(0, self.run_builder(source, first).returncode)
            self.assertEqual(0, self.run_builder(source, second).returncode)

            first_manifest = json.loads((first / "manifest.json").read_text())
            second_manifest = json.loads((second / "manifest.json").read_text())
            self.assertEqual(2, first_manifest["games"]["accepted"])
            self.assertEqual(1, first_manifest["games"]["duplicates_removed"])
            self.assertGreater(first_manifest["decisions"]["accepted"], 0)
            self.assertEqual(
                first_manifest["artifacts"], second_manifest["artifacts"]
            )
            corpus = "".join(path.read_text() for path in first.glob("*.jsonl"))
            self.assertNotIn("remove me", corpus)

            seen: dict[str, str] = {}
            for partition in ("train", "validation", "holdout"):
                for line in (first / f"decisions-{partition}.jsonl").read_text().splitlines():
                    record = json.loads(line)
                    self.assertNotIn(record["position_key"], seen)
                    seen[record["position_key"]] = partition

    def test_refuses_unknown_player_and_existing_output(self) -> None:
        pgn = """[Event "A"]
[White "Someone"]
[Black "Opponent"]
[Result "1-0"]

1. e4 e5 1-0
"""
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / "games.pgn"
            source.write_text(pgn)
            output = root / "output"
            first = self.run_builder(source, output)
            self.assertNotEqual(0, first.returncode)
            self.assertIn("no Fischer games", first.stderr)
            output.mkdir()
            second = self.run_builder(source, output)
            self.assertNotEqual(0, second.returncode)
            self.assertIn("refusing to reuse output directory", second.stderr)


if __name__ == "__main__":
    unittest.main()
