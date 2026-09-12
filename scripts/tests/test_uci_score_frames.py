import asyncio
import unittest

import chess
import chess.engine

from scripts.uci_score_frames import collect_exact_frame


def frame(cp=103, nodes=79459, **extra):
    return {"score": chess.engine.PovScore(chess.engine.Cp(cp), chess.WHITE),
            "wdl": chess.engine.PovWdl(chess.engine.Wdl(535, 465, 0), chess.WHITE),
            "pv": [chess.Move.from_uci("e2e4")], "depth": 14, "nodes": nodes,
            "tbhits": 0, "multipv": 1, **extra}


class ScoreFramesTest(unittest.TestCase):
    def test_bound_and_partial_frames_do_not_overwrite_coherent_exact_label(self):
        exact = frame()
        chosen, final_nodes = collect_exact_frame(iter([
            exact, frame(96, 100041, upperbound=True, depth=15),
            {"depth": 16, "nodes": 100042, "score": chess.engine.PovScore(chess.engine.Cp(12), chess.WHITE)},
        ]))
        self.assertEqual(chosen, exact)
        self.assertEqual(final_nodes, 100042)

    def test_absent_complete_exact_frame_fails_closed(self):
        for frames in ([], [frame(upperbound=True)], [frame(lowerbound=True)],
                       [{"score": frame()["score"]}], [frame(pv=[])],
                       [frame(wdl=None)], [frame(multipv=2)]):
            with self.subTest(frames=frames), self.assertRaises(ValueError):
                collect_exact_frame(frames)

    def test_stream_events_do_not_inherit_aggregated_stale_bounds(self):
        async def posted_events():
            result = chess.engine.AnalysisResult()
            result.post(frame(90, 1000, lowerbound=True))
            result.post(frame(105, 2000))
            result.set_finished(chess.engine.BestMove(chess.Move.from_uci("e2e4"), None))
            self.assertTrue(result.info["lowerbound"])
            return [event async for event in result]
        chosen, nodes = collect_exact_frame(asyncio.run(posted_events()))
        self.assertEqual(chosen["score"].white().score(), 105)
        self.assertNotIn("lowerbound", chosen)
        self.assertEqual(nodes, 2000)

    def test_mate_type_and_snapshot_are_preserved(self):
        original = frame(score=chess.engine.PovScore(chess.engine.Mate(-2), chess.BLACK))
        chosen, _ = collect_exact_frame([original])
        original["pv"].clear()
        self.assertEqual(chosen["pv"], [chess.Move.from_uci("e2e4")])
        self.assertIsNone(chosen["score"].white().score())
        self.assertEqual(chosen["score"].white().mate(), 2)


if __name__ == "__main__":
    unittest.main()
