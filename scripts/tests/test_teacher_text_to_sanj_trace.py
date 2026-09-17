import importlib.util
import tempfile
import unittest
from pathlib import Path


SCRIPT = Path(__file__).resolve().parents[1] / "teacher-text-to-sanj-trace.py"
SPEC = importlib.util.spec_from_file_location("teacher_text_to_trace", SCRIPT)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(MODULE)


class TeacherTextToSanjTraceTests(unittest.TestCase):
    def test_converts_targets_and_never_splits_monotonic_game_runs(self):
        rows = [
            "# neyrang-teacher-wdl-logit400-v2",
            "8/8/8/8/8/8/4K3/7k w - - 0 9 | 0",
            "8/8/8/8/8/8/4K3/7k b - - 1 9 | 400",
            "8/8/8/8/8/8/4K3/7k w - - 0 7 | -400",
        ]
        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory) / "train.txt"
            output = Path(directory) / "trace.tsv"
            source.write_text("\n".join(rows) + "\n", encoding="ascii")
            self.assertEqual((3, 2), MODULE.convert(source, output, "train"))
            converted = output.read_text(encoding="ascii").splitlines()
        self.assertIn(":pair-000001:", converted[0])
        self.assertIn(":pair-000001:", converted[1])
        self.assertIn(":pair-000002:", converted[2])
        self.assertEqual("0.5", converted[0].split("\t")[1])
        self.assertGreater(float(converted[1].split("\t")[1]), 0.5)
        self.assertLess(float(converted[2].split("\t")[1]), 0.5)


if __name__ == "__main__":
    unittest.main()
