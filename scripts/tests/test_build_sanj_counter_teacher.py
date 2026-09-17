import importlib.util
import tempfile
import unittest
from pathlib import Path


SCRIPT = Path(__file__).parents[1] / "build-sanj-counter-teacher.py"
SPEC = importlib.util.spec_from_file_location("build_sanj_counter_teacher", SCRIPT)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(MODULE)


class CounterTeacherTests(unittest.TestCase):
    def test_counter_target_reconstructs_fixed_blend_for_both_sides(self):
        self.assertEqual(MODULE.counter_score(400, 100, "w", 75), 500)
        self.assertEqual(MODULE.counter_score(400, -100, "b", 75), 500)
        self.assertEqual(MODULE.counter_score(3040, -3040, "w", 75), 3040)

    def test_build_rejects_misaligned_features(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            teacher = root / "teacher.txt"
            features = root / "features.tsv"
            output = root / "output.txt"
            teacher.write_text(
                MODULE.TEACHER_HEADER + "\n8/8/8/8/8/8/4K3/7k w - - 0 1 | 0\n"
            )
            features.write_text("fen\tstm\tstm_cp\nother\tw\t6\n")
            with self.assertRaisesRegex(ValueError, "feature FEN differs"):
                MODULE.build(teacher, features, output, 75)


if __name__ == "__main__":
    unittest.main()
