from __future__ import annotations

import hashlib
import importlib.util
import json
import sys
import tempfile
import textwrap
import unittest
from pathlib import Path

import chess


SCRIPT = Path(__file__).resolve().parents[1] / "generate-selfplay-shard.py"


def load_module(name: str, path: Path):
    spec = importlib.util.spec_from_file_location(name, path)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


class SelfPlayShardGeneratorTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.generator = load_module("generate_selfplay_shard", SCRIPT)

    def test_complete_game_is_independently_audited_and_published(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            opening = chess.STARTING_FEN
            openings, opening_manifest = write_opening_shard(root, [opening])
            binary = write_fake_generator(root / "generator", forced_mate_fixture(0))
            output = root / "train-0000.vf"
            partition = self.generator.partition_for_opening(
                opening, "split-seed", 80, 10
            )
            config = make_config(
                self.generator,
                root,
                binary,
                openings,
                opening_manifest,
                output,
                partition=partition,
            )

            manifest = self.generator.generate_shard(config)

            manifest_path = Path(f"{output}.manifest.json")
            self.assertTrue(output.is_file())
            self.assertEqual(json.loads(manifest_path.read_text()), manifest)
            self.assertEqual(manifest["schema"], "neyrang-nnue-selfplay-shard-v1")
            self.assertEqual(manifest["artifact"]["games"], 1)
            self.assertEqual(manifest["artifact"]["scored_positions"], 4)
            self.assertEqual(manifest["audit"]["completion_reasons"], {"checkmate": 1})
            self.assertEqual(manifest["split"]["partition"], partition)
            self.assertEqual(manifest["generator"]["threads"], 1)
            self.assertEqual(manifest["scoring"]["point_of_view"], "White")
            self.assertEqual(manifest["filter"]["version"], "rules-complete-v1")

    def test_wrong_wdl_or_opening_hash_leaves_no_artifact(self) -> None:
        for bad_wdl, corrupt_manifest, expected in [
            (True, False, "WDL"),
            (False, True, "SHA-256"),
        ]:
            with self.subTest(expected=expected), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary)
                opening = chess.STARTING_FEN
                openings, opening_manifest = write_opening_shard(root, [opening])
                if corrupt_manifest:
                    payload = json.loads(opening_manifest.read_text())
                    payload["artifact"]["sha256"] = "0" * 64
                    opening_manifest.write_text(json.dumps(payload), encoding="utf-8")
                binary = write_fake_generator(
                    root / "generator", forced_mate_fixture(2 if bad_wdl else 0)
                )
                output = root / "rejected.vf"
                partition = self.generator.partition_for_opening(
                    opening, "split-seed", 80, 10
                )
                config = make_config(
                    self.generator,
                    root,
                    binary,
                    openings,
                    opening_manifest,
                    output,
                    partition=partition,
                )

                with self.assertRaisesRegex(self.generator.GenerationError, expected):
                    self.generator.generate_shard(config)

                self.assertFalse(output.exists())
                self.assertFalse(Path(f"{output}.manifest.json").exists())

    def test_existing_output_is_never_overwritten(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            opening = chess.STARTING_FEN
            openings, opening_manifest = write_opening_shard(root, [opening])
            binary = write_fake_generator(root / "generator", forced_mate_fixture(0))
            output = root / "occupied.vf"
            output.write_bytes(b"preserve me")
            partition = self.generator.partition_for_opening(
                opening, "split-seed", 80, 10
            )
            config = make_config(
                self.generator,
                root,
                binary,
                openings,
                opening_manifest,
                output,
                partition=partition,
            )

            with self.assertRaisesRegex(self.generator.GenerationError, "refusing to overwrite"):
                self.generator.generate_shard(config)

            self.assertEqual(output.read_bytes(), b"preserve me")

    def test_opening_manifest_count_must_match_the_audited_source(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            opening = chess.STARTING_FEN
            openings, opening_manifest = write_opening_shard(root, [opening])
            payload = json.loads(opening_manifest.read_text())
            payload["artifact"]["openings"] = 2
            opening_manifest.write_text(json.dumps(payload), encoding="utf-8")
            binary = write_fake_generator(root / "generator", forced_mate_fixture(0))
            output = root / "count-mismatch.vf"
            partition = self.generator.partition_for_opening(
                opening, "split-seed", 80, 10
            )
            config = make_config(
                self.generator,
                root,
                binary,
                openings,
                opening_manifest,
                output,
                partition=partition,
            )

            with self.assertRaisesRegex(self.generator.GenerationError, "opening count"):
                self.generator.generate_shard(config)

            self.assertFalse(output.exists())

    def test_registered_legacy_opening_manifest_keeps_original_provenance(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            opening = chess.STARTING_FEN
            openings, opening_manifest = write_opening_shard(root, [opening])
            payload = json.loads(opening_manifest.read_text())
            payload["schema"] = "neyrang-genfens-shard-v1"
            payload["source"] = {
                "kind": "NEYRANG deterministic self-generated openings",
                "license": "UNLICENSED-NEYRANG-INTERNAL",
            }
            opening_manifest.write_text(json.dumps(payload), encoding="utf-8")
            binary = write_fake_generator(root / "generator", forced_mate_fixture(0))
            output = root / "holdout.vf"
            partition = self.generator.partition_for_opening(
                opening, "split-seed", 80, 10
            )

            manifest = self.generator.generate_shard(
                make_config(
                    self.generator,
                    root,
                    binary,
                    openings,
                    opening_manifest,
                    output,
                    partition=partition,
                    source_license="UNLICENSED-NEYRANG-INTERNAL",
                )
            )

            self.assertEqual(manifest["schema"], "neyrang-nnue-selfplay-shard-v1")
            self.assertEqual(
                manifest["opening_source"]["license"],
                "UNLICENSED-NEYRANG-INTERNAL",
            )

    def test_generator_completion_reason_counts_must_match_independent_replay(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            opening = chess.STARTING_FEN
            openings, opening_manifest = write_opening_shard(root, [opening])
            binary = write_fake_generator(
                root / "generator", threefold_fixture(), accepted_positions=8
            )
            output = root / "wrong-reason.vf"
            partition = self.generator.partition_for_opening(
                opening, "split-seed", 80, 10
            )
            config = make_config(
                self.generator,
                root,
                binary,
                openings,
                opening_manifest,
                output,
                partition=partition,
            )

            with self.assertRaisesRegex(self.generator.GenerationError, "completion reasons"):
                self.generator.generate_shard(config)

            self.assertFalse(output.exists())

    def test_color_reversed_openings_share_one_partition_group(self) -> None:
        original = "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq - 0 1"
        mirrored = chess.Board(original).mirror().fen(en_passant="fen")

        self.assertEqual(
            self.generator.opening_group_key(original),
            self.generator.opening_group_key(mirrored),
        )
        self.assertEqual(
            self.generator.partition_for_opening(original, "seed", 80, 10),
            self.generator.partition_for_opening(mirrored, "seed", 80, 10),
        )


def make_config(
    generator,
    root: Path,
    binary: Path,
    openings: Path,
    opening_manifest: Path,
    output: Path,
    **overrides,
):
    values = {
        "repo_root": root,
        "generator": binary,
        "openings": openings,
        "openings_manifest": opening_manifest,
        "output": output,
        "partition": "train",
        "split_seed": "split-seed",
        "train_percent": 80,
        "validation_percent": 10,
        "nodes_per_move": 256,
        "hash_megabytes": 1,
        "maximum_plies": 8,
        "generator_source_commit": "a" * 40,
        "compiler_identity": "rustc 1.98.0 (test)",
        "source_license": "UNLICENSED-NEYRANG-INTERNAL",
        "stall_timeout_seconds": 2.0,
    }
    values.update(overrides)
    return generator.ShardConfig(**values)


def write_opening_shard(root: Path, fens: list[str]) -> tuple[Path, Path]:
    openings = root / "openings.epd"
    data = ("\n".join(fens) + "\n").encode()
    openings.write_bytes(data)
    manifest_path = Path(f"{openings}.manifest.json")
    manifest = {
        "schema": "neyrang-genfens-shard-v1",
        "artifact": {
            "path": openings.relative_to(root).as_posix(),
            "bytes": len(data),
            "sha256": hashlib.sha256(data).hexdigest(),
            "openings": len(fens),
        },
        "source": {
            "kind": "NEYRANG deterministic self-generated openings",
            "license": "UNLICENSED-NEYRANG-INTERNAL",
        },
    }
    manifest_path.write_text(json.dumps(manifest), encoding="utf-8")
    return openings, manifest_path


def write_fake_generator(path: Path, corpus: bytes, accepted_positions: int = 4) -> Path:
    script = textwrap.dedent(
        f"""\
        #!/usr/bin/env python3
        import json
        import pathlib
        import sys

        options = dict(zip(sys.argv[1::2], sys.argv[2::2]))
        openings = pathlib.Path(options["--openings"]).read_text().splitlines()
        pathlib.Path(options["--output"]).write_bytes(bytes({list(corpus)!r}))
        summary = {{
            "schema": "neyrang-selfplay-summary-v1",
            "attempted_games": len(openings),
            "accepted_games": 1,
            "rejected_games": len(openings) - 1,
            "accepted_positions": {accepted_positions},
            "checkmates": 1,
            "stalemates": 0,
            "fifty_move_draws": 0,
            "threefold_draws": 0,
            "rejected_terminal_openings": 0,
            "rejected_maximum_plies": len(openings) - 1,
            "rejected_missing_search_move": 0,
            "rejected_illegal_search_move": 0,
            "rejected_search_mutated_position": 0,
        }}
        pathlib.Path(options["--summary"]).write_text(json.dumps(summary))
        print(
            f"info string selfplay attempted {{len(openings)}} accepted 1 "
            f"rejected {{len(openings) - 1}} positions {accepted_positions}",
            flush=True,
        )
        """
    )
    path.write_text(script, encoding="utf-8")
    path.chmod(0o755)
    return path


def forced_mate_fixture(result: int) -> bytes:
    header = bytearray(
        [
            0xFF, 0xFF, 0, 0, 0, 0, 0xFF, 0xFF,
            0x16, 0x42, 0x25, 0x61, 0, 0, 0, 0,
            0x88, 0x88, 0x88, 0x88, 0x9E, 0xCA, 0xAD, 0xE9,
            0x40, 0, 1, 0, 0, 0, result, 0,
        ]
    )
    records = bytearray()
    for source, destination, score in [
        (13, 21, 11),
        (52, 36, -22),
        (14, 30, -33),
        (59, 31, -29_999),
    ]:
        records.extend((source | (destination << 6)).to_bytes(2, "little"))
        records.extend(score.to_bytes(2, "little", signed=True))
    return bytes(header + records + b"\0\0\0\0")


def threefold_fixture() -> bytes:
    header = bytearray(forced_mate_fixture(1)[:32])
    records = bytearray()
    for source, destination in [
        (6, 21),
        (62, 45),
        (21, 6),
        (45, 62),
        (6, 21),
        (62, 45),
        (21, 6),
        (45, 62),
    ]:
        records.extend((source | (destination << 6)).to_bytes(2, "little"))
        records.extend((0).to_bytes(2, "little", signed=True))
    return bytes(header + records + b"\0\0\0\0")


if __name__ == "__main__":
    unittest.main()
