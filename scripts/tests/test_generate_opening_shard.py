from __future__ import annotations

import importlib.util
import hashlib
import json
import sys
import tempfile
import textwrap
import unittest
from pathlib import Path


SCRIPT = Path(__file__).resolve().parents[1] / "generate-opening-shard.py"


def load_module(name: str, path: Path):
    spec = importlib.util.spec_from_file_location(name, path)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


class OpeningShardGeneratorTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.generator = load_module("generate_opening_shard", SCRIPT)

    def test_valid_engine_output_is_audited_and_published_with_manifest(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            engine = write_fake_engine(root / "engine", VALID_FENS)
            output = root / "shard-0000.epd"
            config = make_config(self.generator, root, engine, output, count=4)

            manifest = self.generator.generate_shard(config)

            manifest_path = Path(f"{output}.manifest.json")
            self.assertTrue(output.is_file())
            self.assertTrue(manifest_path.is_file())
            self.assertEqual(json.loads(manifest_path.read_text()), manifest)
            self.assertEqual(manifest["schema"], "neyrang-genfens-shard-v1")
            self.assertEqual(manifest["artifact"]["openings"], 4)
            self.assertEqual(manifest["audit"]["unique_canonical_positions"], 4)
            self.assertEqual(manifest["generator"]["seed_bits"], 64)
            self.assertEqual(
                manifest["generator"]["command"],
                ["genfens 4 seed 18446744073709551614 book None", "quit"],
            )

    def test_duplicate_or_invalid_output_leaves_no_artifact(self) -> None:
        for lines, error in [
            ([VALID_FENS[0], VALID_FENS[0]], "duplicate canonical"),
            (["not a fen", VALID_FENS[1]], "invalid FEN"),
        ]:
            with self.subTest(error=error), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary)
                engine = write_fake_engine(root / "engine", lines)
                output = root / "rejected.epd"
                config = make_config(self.generator, root, engine, output, count=2)

                with self.assertRaisesRegex(self.generator.GenerationError, error):
                    self.generator.generate_shard(config)

                self.assertFalse(output.exists())
                self.assertFalse(Path(f"{output}.manifest.json").exists())

    def test_opt_in_duplicate_quarantine_retains_first_and_records_indices(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            attempted = [VALID_FENS[0], VALID_FENS[1], VALID_FENS[0], VALID_FENS[2]]
            engine = write_fake_engine(root / "engine", attempted)
            output = root / "deduplicated.epd"
            config = make_config(
                self.generator,
                root,
                engine,
                output,
                count=4,
                quarantine_duplicate_openings=True,
            )

            manifest = self.generator.generate_shard(config)

            self.assertEqual(
                output.read_text(encoding="utf-8").splitlines(),
                [VALID_FENS[0], VALID_FENS[1], VALID_FENS[2]],
            )
            self.assertEqual(manifest["artifact"]["openings"], 3)
            self.assertEqual(manifest["audit"]["unique_canonical_positions"], 3)
            self.assertEqual(manifest["generator"]["attempted_openings"], 4)
            self.assertEqual(manifest["generator"]["last_seed"], 1)
            self.assertEqual(
                manifest["deduplication"],
                {
                    "attempted_openings": 4,
                    "policy": "retain-first-canonical-occurrence-in-seed-order",
                    "quarantined_duplicate_openings": 1,
                    "quarantined_opening_indices": [3],
                    "quarantined_opening_indices_encoding": (
                        "one-based ASCII decimal, one LF-terminated index per line, "
                        "occurrence order"
                    ),
                    "quarantined_opening_indices_sha256": hashlib.sha256(b"3\n").hexdigest(),
                    "retained_openings": 3,
                },
            )

    def test_stalled_engine_is_terminated_without_partial_output(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            engine = write_fake_engine(root / "engine", VALID_FENS[:1], delay=0.2)
            output = root / "stalled.epd"
            config = make_config(
                self.generator,
                root,
                engine,
                output,
                count=1,
                stall_timeout_seconds=0.05,
            )

            with self.assertRaisesRegex(self.generator.GenerationError, "stalled"):
                self.generator.generate_shard(config)

            self.assertFalse(output.exists())
            self.assertFalse(Path(f"{output}.manifest.json").exists())

    def test_existing_output_or_manifest_is_never_overwritten(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            engine = write_fake_engine(root / "engine", VALID_FENS[:1])
            output = root / "occupied.epd"
            output.write_text("preserve me\n", encoding="utf-8")
            config = make_config(self.generator, root, engine, output, count=1)

            with self.assertRaisesRegex(self.generator.GenerationError, "refusing to overwrite"):
                self.generator.generate_shard(config)

            self.assertEqual(output.read_text(encoding="utf-8"), "preserve me\n")

    def test_atomic_link_refuses_a_destination_created_after_preflight(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            temporary_path = root / ".artifact.tmp"
            destination = root / "artifact.epd"
            temporary_path.write_text("new bytes\n", encoding="utf-8")
            destination.write_text("racing writer\n", encoding="utf-8")

            with self.assertRaisesRegex(self.generator.GenerationError, "refusing to overwrite"):
                self.generator.link_without_overwrite(temporary_path, destination)

            self.assertEqual(destination.read_text(encoding="utf-8"), "racing writer\n")
            self.assertEqual(temporary_path.read_text(encoding="utf-8"), "new bytes\n")


def make_config(generator, root: Path, engine: Path, output: Path, **overrides):
    values = {
        "repo_root": root,
        "engine": engine,
        "output": output,
        "count": 4,
        "seed": 18_446_744_073_709_551_614,
        "generator_source_commit": "a" * 40,
        "compiler_identity": "rustc 1.98.0 (test)",
        "source_license": "UNLICENSED-NEYRANG-INTERNAL",
        "stall_timeout_seconds": 15.0,
        "quarantine_duplicate_openings": False,
    }
    values.update(overrides)
    return generator.ShardConfig(**values)


def write_fake_engine(path: Path, fens: list[str], delay: float = 0.0) -> Path:
    script = textwrap.dedent(
        f"""\
        #!/usr/bin/env python3
        import sys
        import time

        expected = f"genfens {len(fens)} seed 18446744073709551614 book None"
        if sys.argv[1:] != [expected, "quit"]:
            print(f"unexpected argv: {{sys.argv[1:]!r}}", file=sys.stderr)
            raise SystemExit(2)
        time.sleep({delay!r})
        for fen in {fens!r}:
            print(f"info string genfens {{fen}}", flush=True)
        """
    )
    path.write_text(script, encoding="utf-8")
    path.chmod(0o755)
    return path


VALID_FENS = [
    "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
    "rnbqkbnr/pppppppp/8/8/8/5N2/PPPPPPPP/RNBQKB1R b KQkq - 1 1",
    "r1bqkbnr/pppppppp/2n5/8/8/5N2/PPPPPPPP/RNBQKB1R w KQkq - 2 2",
    "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq - 0 1",
]


if __name__ == "__main__":
    unittest.main()
