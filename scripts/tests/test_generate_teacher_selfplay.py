import tempfile
import unittest
from pathlib import Path

from scripts.generate_teacher_selfplay import (
    Config,
    choose_line,
    partition,
    select_records,
    stable_random,
    validate,
)


class TeacherSelfplayTest(unittest.TestCase):
    def test_seeded_partition_and_choice_are_reproducible(self):
        self.assertEqual(
            [partition("fixed", game) for game in range(100)],
            [partition("fixed", game) for game in range(100)],
        )
        infos = [{"rank": rank} for rank in range(4)]
        self.assertEqual(
            [choose_line(infos, stable_random("fixed", game))["rank"] for game in range(32)],
            [choose_line(infos, stable_random("fixed", game))["rank"] for game in range(32)],
        )
        self.assertEqual(
            {partition("fixed", game) for game in range(100)},
            {"train", "validation", "holdout"},
        )

    def test_contract_rejects_unsafe_or_reused_outputs(self):
        with tempfile.TemporaryDirectory() as root:
            root = Path(root)
            teacher = root / "teacher"
            teacher.write_bytes(b"fixture")
            base = dict(
                teacher=str(teacher),
                output=str(root / "output"),
                games=8,
                workers=2,
                nodes=100,
                hash_mb=16,
                max_plies=80,
                variation_plies=12,
                multipv=4,
                batch_size=4,
                max_positions_per_game=4,
                max_saturated_per_game=1,
                seed="fixed",
            )
            validate(Config(**base))
            for key, value in [
                ("games", 0),
                ("workers", 9),
                ("multipv", 9),
                ("max_saturated_per_game", 5),
            ]:
                invalid = base | {key: value}
                with self.assertRaises(ValueError):
                    validate(Config(**invalid))
            (root / "output").mkdir()
            with self.assertRaises(FileExistsError):
                validate(Config(**base))

    def test_per_game_selection_is_bounded_and_reproducible(self):
        records = [
            (ply, f"fen-{ply}", 3040 if ply % 2 else ply * 10)
            for ply in range(20)
        ]
        selected = select_records(records, "fixed", 7, 6, 2)
        self.assertEqual(selected, select_records(records, "fixed", 7, 6, 2))
        self.assertEqual(len(selected), 6)
        self.assertLessEqual(sum(abs(score) == 3040 for _, _, score in selected), 2)
        self.assertEqual(selected, sorted(selected))


if __name__ == "__main__":
    unittest.main()
