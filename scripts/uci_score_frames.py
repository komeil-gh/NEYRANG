"""Select coherent score frames from a python-chess analysis event stream."""

from copy import deepcopy


def collect_exact_frame(frames):
    """Return a score-frame snapshot and independent maximum reported work.

    Consume analysis events, never the merged AnalysisResult.info dictionary.
    The chosen frame may precede the node cutoff and final bestmove.
    """
    chosen = None
    final_nodes = 0
    for frame in frames:
        nodes = frame.get("nodes")
        if isinstance(nodes, int):
            final_nodes = max(final_nodes, nodes)
        if any(frame.get(key) is None for key in
               ("score", "wdl", "pv", "depth", "nodes", "tbhits")):
            continue
        if (frame.get("lowerbound") or frame.get("upperbound")
                or frame.get("multipv", 1) != 1
                or frame["depth"] <= 0 or frame["nodes"] <= 0
                or not frame["pv"]):
            continue
        chosen = deepcopy(frame)
    if chosen is None:
        raise ValueError("No complete unbounded principal score frame")
    return chosen, final_nodes
