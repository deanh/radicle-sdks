"""Read-only warm-idle measurements for state, full-ref and sigrefs prototypes."""

import argparse
import json
from pathlib import Path
import statistics
import tempfile
import time

import radiclepy as rad
from operation_events import poll_operations
from watch_issues import poll_once


def measure(profile, rid, rounds=5):
    """Use fresh external cursors, baseline once, then time unchanged polls.

    Caller must keep the repository idle. These measurements are not backend
    I/O tracing, cold-cache benchmarks or performance guarantees.
    """
    result = {"rid": rid, "rounds": rounds, "models": {}}
    with tempfile.TemporaryDirectory(prefix="radicle-event-bench-") as temporary:
        for model in ("state", "scan", "sigrefs"):
            path = Path(temporary) / f"{model}.json"

            def poll():
                if model == "state":
                    count = poll_once(profile, rid, path, lambda event: None)
                    return {"events": count, "calls": {"typed.issues.all": 1}}
                return poll_operations(profile, rid, path, lambda event: None, source=model)

            poll()  # silent baseline, outside timing
            samples = []
            stats = None
            for _ in range(rounds):
                started = time.perf_counter()
                stats = poll()
                samples.append((time.perf_counter() - started) * 1000)
                if stats["events"]:
                    raise RuntimeError("repository changed during idle benchmark")
            cursor = json.loads(path.read_text())
            result["models"][model] = {
                "medianMs": statistics.median(samples), "maxMs": max(samples),
                "cursorBytes": len(json.dumps(cursor, separators=(",", ":")).encode()),
                "lastPoll": stats,
            }
    return result


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("rid")
    parser.add_argument("--rounds", type=int, default=5)
    args = parser.parse_args()
    if not 1 <= args.rounds <= 100:
        parser.error("rounds must be between 1 and 100")
    print(json.dumps(measure(rad.Profile.load(), args.rid, args.rounds), indent=2))
