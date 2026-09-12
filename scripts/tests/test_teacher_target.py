import math
import unittest

from scripts.teacher_target import wdl_score


class TeacherTargetTest(unittest.TestCase):
    def test_endpoints_neutral_and_known_value(self):
        self.assertEqual(wdl_score([1000, 0, 0]), 3040)
        self.assertEqual(wdl_score([0, 0, 1000]), -3040)
        self.assertEqual(wdl_score([0, 1000, 0]), 0)
        self.assertEqual(wdl_score([700, 200, 100]), 555)

    def test_entire_expectation_grid_is_monotone_symmetric_and_bounded(self):
        previous = -32768
        for numerator in range(2001):
            wins, draws = divmod(numerator, 2)
            losses = 1000 - wins - draws
            score = wdl_score([wins, draws, losses])
            self.assertGreaterEqual(score, previous)
            self.assertEqual(score, -wdl_score([losses, draws, wins]))
            self.assertLessEqual(abs(score), 3040)
            expected = max(1, min(1999, numerator)) / 2000
            decoded = 1 / (1 + math.exp(-score / 400))
            self.assertLessEqual(abs(decoded - expected), 0.0003125)
            previous = score

    def test_malformed_probability_vectors_fail_closed(self):
        for value in ([], [500, 500], [0, 0, 0], [-1, 1, 1000], [1001, 0, -1],
                      [True, 999, 0], [700.0, 200, 100], [float("nan"), 0, 1000],
                      "700,200,100", None):
            with self.subTest(value=value), self.assertRaises(ValueError):
                wdl_score(value)


if __name__ == "__main__":
    unittest.main()
