"""Warm, read-only reconciliation cost probe through installed public exports."""

import argparse
import json
from pathlib import Path
import statistics
import time

import radiclepy as rad

from reconcile_refs import emit_cursor, since


def measure(repository, *, rounds=5, cursor=None):
    if rounds < 1:
        raise ValueError("rounds must be positive")
    tips = {tip.node_id: tip.target for tip in repository.namespace_tips()}
    if cursor is None:
        cursor = emit_cursor(repository.rid, tips)
    result = {"rid": repository.rid, "namespaces": len(tips),
              "cursorBytes": len(cursor.encode("utf-8")), "rounds": rounds, "models": {}}
    pages = []
    for name, full in [("tips", False), ("signed-set-scan", True)]:
        since(repository, cursor, full_scan=full)  # warm-up
        elapsed = []
        for _ in range(rounds):
            start = time.perf_counter()
            page, stats = since(repository, cursor, full_scan=full)
            elapsed.append((time.perf_counter() - start) * 1000)
        result["models"][name] = {"medianMs": statistics.median(elapsed), "stats": stats,
            "namespaceRecords": len(page["namespaces"]), "cobRecords": len(page["cobs"]),
            "incomplete": page["incomplete"]}
        pages.append(page)
    if pages[0] != pages[1]:
        raise RuntimeError("tip comparison and full signed-set scan disagree")
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("rid")
    parser.add_argument("--rounds", type=int, default=5)
    parser.add_argument("--since", help="optional external cursor for a delta/resume probe")
    args = parser.parse_args()
    cursor = Path(args.since).read_bytes().decode("utf-8") if args.since else None
    print(json.dumps(measure(rad.Profile.load().storage.repository(args.rid),
                             rounds=args.rounds, cursor=cursor), sort_keys=True))


if __name__ == "__main__":
    main()
