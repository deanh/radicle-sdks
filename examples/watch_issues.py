"""Read-only local issue/comment observer spike; see watch_issues.md.

No signer, node connection or reply bot. Only the caller-owned checkpoint is
written. Import poll_once to supply a synchronous application callback.
"""

import argparse
import inspect
import json
import math
import os
from pathlib import Path
import sys
import tempfile
import time

import radiclepy as rad


def checkpoint_path(profile, filename):
    path = Path(filename).absolute()
    if path.is_symlink():
        raise ValueError("checkpoint must not be a symlink")
    path = path.parent.resolve(strict=True) / path.name
    for root in (profile.home.path, profile.storage.path):
        if path.is_relative_to(Path(root).resolve(strict=True)):
            raise ValueError("checkpoint must be outside Radicle home and storage")
    if path.exists() and not path.is_file():
        raise ValueError("checkpoint must be a regular file")
    return path


def load_checkpoint(path, storage, rid):
    try:
        text = path.read_text(encoding="utf-8")
    except FileNotFoundError:
        return None
    state = json.loads(text)
    if (not isinstance(state, dict)
            or set(state) != {"version", "storage", "rid", "seen"}
            or isinstance(state["version"], bool) or state["version"] != 1
            or state["storage"] != storage or state["rid"] != rid
            or not isinstance(state["seen"], dict)):
        raise ValueError("invalid checkpoint version, scope or shape")
    for issue_id, comments in state["seen"].items():
        if rad.parse_object_id(issue_id) != issue_id or not isinstance(comments, list):
            raise ValueError("invalid checkpoint issue/comments")
        if any(not isinstance(cid, str) or rad.parse_oid(cid) != cid for cid in comments):
            raise ValueError("invalid checkpoint comment ID")
        if len(set(comments)) != len(comments):
            raise ValueError("duplicate checkpoint comment ID")
    return state


def save_checkpoint(path, state):
    # Same-directory replace is atomic on our validated POSIX platform. This is
    # not a directory-fsync durability guarantee or a concurrent-writer lock.
    fd, temporary = tempfile.mkstemp(prefix=f".{path.name}.", dir=path.parent)
    try:
        with os.fdopen(fd, "w", encoding="utf-8") as stream:
            json.dump(state, stream, sort_keys=True, ensure_ascii=True)
            stream.write("\n")
            stream.flush()
            os.fsync(stream.fileno())
        os.replace(temporary, path)
    finally:
        try:
            os.unlink(temporary)
        except FileNotFoundError:
            pass


def poll_once(profile, rid, filename, handle, *, replay=False):
    """Read one batch, synchronously deliver it, then checkpoint. Return its size.

    First scan is a silent baseline unless replay=True. The flag only affects a
    missing checkpoint. Callback/checkpoint failure can replay the entire batch.
    """
    if not callable(handle):
        raise TypeError("handle must be a synchronous callable")
    rid = rad.parse_repo_id(rid)
    path = checkpoint_path(profile, filename)
    storage = str(Path(profile.storage.path).resolve(strict=True))
    previous = load_checkpoint(path, storage, rid)
    seen = {} if previous is None else {
        iid: set(comments) for iid, comments in previous["seen"].items()
    }
    # Materialize all reads before calling application code. A read failure
    # delivers nothing and advances nothing; this is not a transaction snapshot.
    issues = sorted(rad.Issues.open(profile.storage.repository(rid)).all(), key=lambda i: i.id)
    events = []
    for issue in issues:
        known = seen.get(issue.id)
        if known is None:
            known = set()
            events.append({
                "id": f"{rid}/issue/{issue.id}", "type": "issue.discovered",
                "rid": rid, "issueId": issue.id, "title": issue.title,
                "author": issue.author,
            })
        root = issue.comments[0].id if issue.comments else None
        for comment in issue.comments:
            if comment.id not in known and comment.id != root:
                events.append({
                    "id": f"{rid}/issue/{issue.id}/comment/{comment.id}",
                    "type": "comment.observed", "rid": rid, "issueId": issue.id,
                    "title": issue.title, "commentId": comment.id,
                    "author": comment.author, "body": comment.body,
                })
            known.add(comment.id)
        seen[issue.id] = known
    state = {
        "version": 1, "storage": storage, "rid": rid,
        "seen": {iid: sorted(seen[iid]) for iid in sorted(seen)},
    }
    if previous is None and not replay:
        events = []
    for event in events:
        result = handle(event)
        if inspect.isawaitable(result):
            if inspect.iscoroutine(result):
                result.close()
            raise TypeError("handle must be synchronous, not an async callback")
    if state != previous:
        save_checkpoint(path, state)
    return len(events)


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("rid", help="one locally stored repository ID")
    parser.add_argument("--state", required=True, help="checkpoint file; parent must exist")
    parser.add_argument("--once", action="store_true", help="scan once and exit")
    parser.add_argument("--replay", action="store_true", help="emit existing data on a fresh checkpoint")
    parser.add_argument("--interval", type=float, default=2, help="poll seconds, default 2")
    args = parser.parse_args(argv)
    if not math.isfinite(args.interval) or not 0 < args.interval <= 86400:
        parser.error("interval must be finite, > 0 and <= 86400 seconds")
    profile = rad.Profile.load()
    while True:
        poll_once(profile, args.rid, args.state,
                  lambda event: print(json.dumps(event, sort_keys=True), flush=True),
                  replay=args.replay)
        if args.once:
            return 0
        time.sleep(args.interval)


if __name__ == "__main__":
    try:
        sys.exit(main())
    except KeyboardInterrupt:
        sys.exit(130)
    except Exception as error:
        print(f"observer failed: {error}", file=sys.stderr)
        sys.exit(1)
