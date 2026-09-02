from __future__ import annotations

import importlib.util
import json
import shutil
import sys
import tempfile
import unittest
from pathlib import Path

import chess
import chess.pgn


SCRIPTS = Path(__file__).resolve().parents[1]
BUILDER_PATH = SCRIPTS / "build-sanj-corpus.py"
AUDITOR_PATH = SCRIPTS / "audit-sanj-corpus.py"


def load_module(name: str, path: Path):
    spec = importlib.util.spec_from_file_location(name, path)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


builder = load_module("build_eval_corpus_for_audit_test", BUILDER_PATH)
auditor = load_module("audit_eval_corpus", AUDITOR_PATH)


class CorpusAuditorTests(unittest.TestCase):
    def test_independent_replay_groups_concurrent_completion_by_round(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            source = root / "concurrent.pgn"
            for index, (round_number, white, black) in enumerate(
                (
                    ("2", "Engine-A", "Engine-B"),
                    ("1", "Engine-A", "Engine-B"),
                    ("2", "Engine-B", "Engine-A"),
                    ("1", "Engine-B", "Engine-A"),
                )
            ):
                write_game(
                    source,
                    white,
                    black,
                    ("g1f3", "g8f6", "b1c3", "b8c6"),
                    append=index > 0,
                    round_number=round_number,
                )
            output = root / "corpus"
            builder.build_corpus(
                builder.BuildConfig(
                    repo_root=root,
                    output_dir=output,
                    sources=(builder.SourceSpec("fixture", source),),
                    seed="auditor-concurrent-test",
                    min_ply=1,
                    tail_plies=0,
                )
            )
            shutil.copyfile(BUILDER_PATH, root / BUILDER_PATH.name)

            summary = auditor.audit_corpus(root, output)

            self.assertTrue(summary["ok"])
            self.assertEqual(summary["sources"]["fixture"]["pairs_parsed"], 2)

    def test_independent_replay_accepts_fixed_holdout_partition(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root, output = build_fixture(
                Path(temporary), fixed_partition="holdout"
            )
            try:
                summary = auditor.audit_corpus(root, output)
            except auditor.AuditError as error:
                self.fail(f"fixed holdout replay was rejected: {error}")

            self.assertTrue(summary["ok"])
            self.assertEqual(summary["records"]["train"], 0)
            self.assertEqual(summary["records"]["validation"], 0)
            self.assertGreater(summary["records"]["holdout"], 0)

    def test_independent_replay_accepts_valid_corpus(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root, output = build_fixture(Path(temporary))
            summary = auditor.audit_corpus(root, output)
            self.assertTrue(summary["ok"])
            self.assertEqual(summary["unique_record_ids"], 2)
            self.assertEqual(summary["unique_position_keys"], 2)

    def test_independent_replay_accepts_registered_legacy_single_sample_manifest(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root, output = build_fixture(Path(temporary))
            manifest_path = output / "manifest.json"
            manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
            for field in (
                "mode",
                "selector_schema",
                "min_sample_gap_plies",
                "pair_balanced_record_count",
            ):
                manifest["sampling"].pop(field)
            for source in manifest["sources"]:
                source.pop("pair_record_counts")
                source.pop("target_record_counts")
            manifest_path.write_text(json.dumps(manifest), encoding="utf-8")

            try:
                summary = auditor.audit_corpus(root, output)
            except auditor.AuditError as error:
                self.fail(f"registered legacy manifest was rejected: {error}")

            self.assertTrue(summary["ok"])

    def test_independent_replay_accepts_dense_pair_balanced_corpus(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root, output = build_fixture(Path(temporary), dense=True)
            try:
                summary = auditor.audit_corpus(root, output)
            except auditor.AuditError as error:
                self.fail(f"dense independent replay was rejected: {error}")
            self.assertTrue(summary["ok"])
            self.assertEqual(summary["sources"]["fixture"]["records_sampled"], 4)

    def test_replay_rejects_rehashed_tsv_tampering(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root, output = build_fixture(Path(temporary))
            manifest_path = output / "manifest.json"
            manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
            partition = next(
                name
                for name in auditor.PARTITIONS
                if manifest["records"]["partitions"][name]["records"]
            )
            artifact = output / f"{partition}.input.tsv"
            text = artifact.read_text(encoding="utf-8")
            artifact.write_text(text.replace("\t0.5\t", "\t1\t", 1), encoding="utf-8")
            digest, byte_count = auditor.sha256_file(artifact)
            entry = manifest["records"]["partitions"][partition]
            entry["sha256"] = digest
            entry["bytes"] = byte_count
            manifest_path.write_text(json.dumps(manifest), encoding="utf-8")

            with self.assertRaisesRegex(
                auditor.AuditError, "TSV differs from independent replay"
            ):
                auditor.audit_corpus(root, output)

    def test_replay_rejects_pair_record_summary_tampering(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root, output = build_fixture(Path(temporary), dense=True)
            manifest_path = output / "manifest.json"
            manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
            manifest["sources"][0]["pair_record_counts"] = {"2": 999}
            manifest_path.write_text(json.dumps(manifest), encoding="utf-8")

            with self.assertRaisesRegex(
                auditor.AuditError, "pair_record_counts differs"
            ):
                auditor.audit_corpus(root, output)


def build_fixture(
    root: Path,
    dense: bool = False,
    fixed_partition: str | None = None,
) -> tuple[Path, Path]:
    source = root / "games.pgn"
    write_game(
        source,
        "Engine-A",
        "Engine-B",
        ("g1f3", "g8f6", "b1c3", "b8c6"),
        append=False,
    )
    write_game(
        source,
        "Engine-B",
        "Engine-A",
        ("e2e4", "e7e5", "g1f3", "b8c6"),
        append=True,
    )
    output = root / "corpus"
    config = builder.BuildConfig(
        repo_root=root,
        output_dir=output,
        sources=(builder.SourceSpec("fixture", source),),
        seed="auditor-test",
        min_ply=1,
        tail_plies=0,
        samples_per_game=2 if dense else 1,
        min_sample_gap=2 if dense else 0,
        fixed_partition=fixed_partition,
    )
    builder.build_corpus(config)
    shutil.copyfile(BUILDER_PATH, root / BUILDER_PATH.name)
    return root, output


def write_game(
    path: Path,
    white: str,
    black: str,
    moves: tuple[str, ...],
    append: bool,
    round_number: str = "1",
) -> None:
    game = chess.pgn.Game()
    game.headers.update(
        {
            "Event": "Auditor fixture",
            "Round": round_number,
            "White": white,
            "Black": black,
            "Result": "1/2-1/2",
            "Termination": "normal",
            "TimeControl": "1+0.01",
            "PlyCount": "4",
        }
    )
    board = game.board()
    node = game
    for notation in moves:
        move = chess.Move.from_uci(notation)
        if move not in board.legal_moves:
            raise AssertionError(f"fixture move {notation} is illegal")
        node = node.add_variation(move)
        board.push(move)
    exporter = chess.pgn.StringExporter(headers=True, variations=False, comments=False)
    with path.open("a" if append else "w", encoding="utf-8", newline="\n") as handle:
        handle.write(game.accept(exporter))
        handle.write("\n\n")


if __name__ == "__main__":
    unittest.main()
