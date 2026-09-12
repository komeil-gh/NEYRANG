"""Versioned probability encoding, not material centipawns or a dataset exporter."""

import math

ENCODING = "neyrang-teacher-wdl-logit400-v1"


def wdl_score(wdl_white):
    """Encode White's per-mille WDL for Bullet's score-only sigmoid(s / 400).

    Clip expectations to [1/2000, 1999/2000], then round logit units to the
    nearest integer (ties to even). Outcome and row eligibility stay separate.
    """
    if (not isinstance(wdl_white, (list, tuple)) or len(wdl_white) != 3
            or any(type(n) is not int or not 0 <= n <= 1000 for n in wdl_white)
            or sum(wdl_white) != 1000):
        raise ValueError("WDL must contain three integer per-mille counts summing to 1000")
    numerator = max(1, min(1999, 2 * wdl_white[0] + wdl_white[1]))
    return round(400 * math.log(numerator / (2000 - numerator)))
