import importlib.util
import unittest
from pathlib import Path


SCRIPT = Path(__file__).parents[1] / "fit-sanj-psqt-residual.py"
SPEC = importlib.util.spec_from_file_location("fit_sanj_psqt_residual", SCRIPT)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(MODULE)


class PsqtResidualTests(unittest.TestCase):
    def test_horizontal_mirror_and_color_flip_share_features(self):
        white_a = MODULE.board_coefficients("8/8/8/8/8/8/N7/8")
        white_h = MODULE.board_coefficients("8/8/8/8/8/8/7N/8")
        black_a = MODULE.board_coefficients("8/n7/8/8/8/8/8/8")
        self.assertEqual(white_a, white_h)
        self.assertEqual(white_a, {index: -value for index, value in black_a.items()})

    def test_invalid_rank_width_fails_closed(self):
        with self.assertRaisesRegex(ValueError, "invalid FEN board"):
            MODULE.board_coefficients("8/8/8/8/8/8/8/7")


if __name__ == "__main__":
    unittest.main()
