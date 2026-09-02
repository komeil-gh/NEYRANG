from __future__ import annotations

import importlib.util
import json
import sys
import tempfile
import unittest
from pathlib import Path

import chess
import chess.pgn


SCRIPT = Path(__file__).resolve().parents[1] / "build-sanj-corpus.py"
SPEC = importlib.util.spec_from_file_location("build_eval_corpus", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
corpus = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = corpus
SPEC.loader.exec_module(corpus)


class CorpusBuilderTests(unittest.TestCase):
    def test_fixed_partition_places_every_record_in_sealed_holdout(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            source = root / "games.pgn"
            write_pair(source, "Engine-A", "Engine-B")
            base = make_config(root, root / "output", source)

            self.assertTrue(hasattr(base, "fixed_partition"))
            config = corpus.BuildConfig(
                **{**base.__dict__, "fixed_partition": "holdout"}
            )
            manifest = corpus.build_corpus(config)

            counts = manifest["records"]["partitions"]
            self.assertEqual(counts["train"]["records"], 0)
            self.assertEqual(counts["validation"]["records"], 0)
            self.assertGreater(counts["holdout"]["records"], 0)
            self.assertEqual(
                counts["holdout"]["records"],
                manifest["records"]["deduplication"]["records_kept"],
            )
            self.assertEqual(
                manifest["split"],
                {
                    "mode": "fixed-partition-v1",
                    "group_key": "canonical first-four-field starting FEN",
                    "partition": "holdout",
                    "seed": "20260830",
                },
            )

    def test_fixed_partition_rejects_unknown_partition(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            source = root / "games.pgn"
            write_pair(source, "Engine-A", "Engine-B")
            base = make_config(root, root / "output", source)
            config = corpus.BuildConfig(
                **{**base.__dict__, "fixed_partition": "sealed"}
            )

            with self.assertRaisesRegex(corpus.CorpusError, "fixed partition"):
                corpus.validate_config(config)

    def test_sampling_configuration_is_explicit(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            source = root / "games.pgn"
            write_pair(source, "Engine-A", "Engine-B")
            config = make_config(root, root / "output", source)

            self.assertTrue(hasattr(config, "samples_per_game"))
            self.assertTrue(hasattr(config, "min_sample_gap"))
            self.assertEqual(getattr(config, "samples_per_game", None), 1)
            self.assertEqual(getattr(config, "min_sample_gap", None), 0)

    def test_sampling_configuration_rejects_ambiguous_modes(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            source = root / "games.pgn"
            write_pair(source, "Engine-A", "Engine-B")
            base = make_config(root, root / "output", source)

            invalid = (
                ({"samples_per_game": 0}, "samples-per-game must be positive"),
                ({"min_sample_gap": -1}, "min-sample-gap must be non-negative"),
                (
                    {"samples_per_game": 1, "min_sample_gap": 1},
                    "single-sample mode requires min-sample-gap 0",
                ),
                (
                    {"samples_per_game": 2, "min_sample_gap": 0},
                    "dense mode requires a positive min-sample-gap",
                ),
            )
            for changes, message in invalid:
                config = corpus.BuildConfig(**{**base.__dict__, **changes})
                with self.subTest(changes=changes):
                    with self.assertRaisesRegex(corpus.CorpusError, message):
                        corpus.validate_config(config)

    def test_dense_sampling_is_gap_constrained_and_pair_balanced(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            source = root / "dense.pgn"
            write_custom_game(
                source,
                "Engine-A",
                "Engine-B",
                (
                    "g1f3",
                    "g8f6",
                    "b1c3",
                    "b8c6",
                    "a2a3",
                    "a7a6",
                    "h2h3",
                    "h7h6",
                    "b2b3",
                    "b7b6",
                    "g2g3",
                    "g7g6",
                    "f1g2",
                    "f8g7",
                    "d2d3",
                    "d7d6",
                ),
                append=False,
            )
            write_custom_game(
                source,
                "Engine-B",
                "Engine-A",
                ("e2e4", "e7e5", "g1f3", "b8c6"),
                append=True,
            )
            base = make_config(root, root / "dense-output", source)
            config = corpus.BuildConfig(
                **{
                    **base.__dict__,
                    "samples_per_game": 3,
                    "min_sample_gap": 2,
                }
            )

            manifest = corpus.build_corpus(config)
            counts = manifest["sources"][0]["counts"]
            self.assertEqual(counts["records_sampled"], 4)
            self.assertEqual(counts.get("pair_balancing_records_dropped"), 1)
            self.assertEqual(
                manifest["sources"][0].get("pair_record_counts"), {"4": 1}
            )
            self.assertEqual(
                manifest["sources"][0].get("target_record_counts"), {"0.5": 4}
            )
            sampling = manifest["sampling"]
            self.assertEqual(sampling.get("mode"), "pair-balanced-gap-hash-v1")
            self.assertEqual(sampling["max_positions_per_game"], 3)
            self.assertEqual(sampling.get("min_sample_gap_plies"), 2)

            by_game: dict[int, list[int]] = {1: [], 2: []}
            for partition in corpus.PARTITIONS:
                lines = (config.output_dir / f"{partition}.input.tsv").read_text().splitlines()
                for line in lines[1:]:
                    record_id = line.split("\t", 1)[0]
                    game = int(record_id.split(":game-")[1].split(":", 1)[0])
                    ply = int(record_id.rsplit(":ply-", 1)[1])
                    by_game[game].append(ply)
            self.assertEqual({game: len(plies) for game, plies in by_game.items()}, {1: 2, 2: 2})
            for plies in by_game.values():
                ordered = sorted(plies)
                self.assertTrue(
                    all(right - left >= 2 for left, right in zip(ordered, ordered[1:]))
                )

    def test_build_is_deterministic_and_groups_reused_openings(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            first_source = root / "first.pgn"
            second_source = root / "second.pgn"
            write_pair(first_source, "Engine-A", "Engine-B")
            write_pair(second_source, "Engine-C", "Engine-D")

            first = root / "first-output"
            second = root / "second-output"
            config = make_config(root, first, first_source, second_source)
            first_manifest = corpus.build_corpus(config)
            second_manifest = corpus.build_corpus(
                corpus.BuildConfig(**{**config.__dict__, "output_dir": second})
            )

            for partition in corpus.PARTITIONS:
                self.assertEqual(
                    (first / f"{partition}.input.tsv").read_bytes(),
                    (second / f"{partition}.input.tsv").read_bytes(),
                )
            self.assertEqual(normalize(first_manifest), normalize(second_manifest))

            populated = [
                name
                for name in corpus.PARTITIONS
                if first_manifest["records"]["partitions"][name]["records"]
            ]
            self.assertEqual(len(populated), 1)
            self.assertEqual(
                first_manifest["records"]["sampled_before_deduplication"],
                4,
            )
            self.assertGreater(
                first_manifest["records"]["partitions"][populated[0]]["records"], 0
            )
            verify_artifacts(first, first_manifest)

            with self.assertRaisesRegex(corpus.CorpusError, "refusing to overwrite"):
                corpus.build_corpus(config)

    def test_odd_and_non_reversed_sources_fail_closed(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            odd = root / "odd.pgn"
            write_game(odd, "Engine-A", "Engine-B", append=False)
            with self.assertRaisesRegex(corpus.CorpusError, "odd trailing game"):
                corpus.build_corpus(make_config(root, root / "odd-output", odd))

            invalid = root / "invalid.pgn"
            write_game(invalid, "Engine-A", "Engine-B", append=False)
            write_game(invalid, "Engine-A", "Engine-B", append=True)
            with self.assertRaisesRegex(corpus.CorpusError, "colors are not reversed"):
                corpus.build_corpus(make_config(root, root / "invalid-output", invalid))

    def test_quiet_filter_rejects_check_capture_and_promotion(self) -> None:
        self.assertFalse(corpus.has_legal_tactical_move(chess.Board()))

        capture = chess.Board("7k/8/8/8/8/8/4Q3/4r2K w - - 0 1")
        self.assertTrue(corpus.has_legal_tactical_move(capture))

        promotion = chess.Board("7k/P7/8/8/8/8/8/7K w - - 0 1")
        self.assertTrue(corpus.has_legal_tactical_move(promotion))

        check = chess.Board("7k/8/8/8/8/8/7r/7K w - - 0 1")
        self.assertTrue(check.is_check())

    def test_cross_partition_transpositions_are_removed(self) -> None:
        fen = chess.STARTING_FEN
        key = corpus.canonical_position_key(fen)
        left = corpus.Record("train", "a", 0.5, fen, key, "s", 1, 1, 0)
        right = corpus.Record("holdout", "b", 0.5, fen, key, "s", 2, 1, 0)
        kept, summary = corpus.deduplicate_records([left, right])
        self.assertEqual(kept, [])
        self.assertEqual(summary["cross_partition_keys_removed"], 1)
        self.assertEqual(summary["cross_partition_records_removed"], 2)


def make_config(
    root: Path, output: Path, *sources: Path
) -> corpus.BuildConfig:
    return corpus.BuildConfig(
        repo_root=root,
        output_dir=output,
        sources=tuple(
            corpus.SourceSpec(f"source-{index}", source)
            for index, source in enumerate(sources, start=1)
        ),
        seed="20260830",
        min_ply=0,
        tail_plies=0,
    )


def write_pair(path: Path, first: str, second: str) -> None:
    write_game(path, first, second, append=False)
    write_game(path, second, first, append=True)


def write_game(path: Path, white: str, black: str, append: bool) -> None:
    write_custom_game(
        path,
        white,
        black,
        ("g1f3", "g8f6", "b1c3", "b8c6"),
        append,
    )


def write_custom_game(
    path: Path,
    white: str,
    black: str,
    moves: tuple[str, ...],
    append: bool,
) -> None:
    game = chess.pgn.Game()
    game.headers.update(
        {
            "Event": "Corpus fixture",
            "Round": "1",
            "White": white,
            "Black": black,
            "Result": "1/2-1/2",
            "Termination": "normal",
            "TimeControl": "1+0.01",
            "PlyCount": str(len(moves)),
        }
    )
    board = game.board()
    node = game
    for notation in moves:
        move = chess.Move.from_uci(notation)
        self_legal = move in board.legal_moves
        if not self_legal:
            raise AssertionError(f"fixture move {notation} is illegal")
        node = node.add_variation(move)
        board.push(move)
    exporter = chess.pgn.StringExporter(headers=True, variations=False, comments=False)
    mode = "a" if append else "w"
    with path.open(mode, encoding="utf-8", newline="\n") as handle:
        handle.write(game.accept(exporter))
        handle.write("\n\n")


def normalize(manifest: dict) -> dict:
    value = json.loads(json.dumps(manifest))
    value.pop("created_utc", None)
    value.pop("output_dir", None)
    return value


def verify_artifacts(directory: Path, manifest: dict) -> None:
    for partition in corpus.PARTITIONS:
        expected = manifest["records"]["partitions"][partition]
        digest, byte_count = corpus.sha256_file(directory / expected["path"])
        self_digest = expected["sha256"]
        if digest != self_digest or byte_count != expected["bytes"]:
            raise AssertionError(f"artifact mismatch for {partition}")


if __name__ == "__main__":
    unittest.main()
