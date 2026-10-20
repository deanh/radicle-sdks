"""Application reconciliation experiment, not an SDK feed API. See reconciliation.md."""

import argparse
import inspect
import json
import os
from pathlib import Path
import re
import sys
import tempfile

import radiclepy as rad

from watch_issues import checkpoint_path

HEADER = "# radicle-events v1 "
REF = re.compile(r"refs/namespaces/([^/]+)/refs/rad/sigrefs")
OID = re.compile(r"[0-9a-f]{40}")


def parse_cursor(text, rid):
    """Strict canonical text; never silently normalize an external checkpoint."""
    rad.parse_repo_id(rid)
    lines = text.split("\n")
    if not lines or lines[0] != HEADER + rid:
        raise ValueError("cursor line 1: wrong version, repository or header")
    if lines[-1] != "":
        raise ValueError(f"cursor line {len(lines)}: missing final LF")
    tips, previous = {}, ""
    for index, line in enumerate(lines[1:-1], 2):
        parts = line.split(" ")
        if len(parts) != 2:
            raise ValueError(f"cursor line {index}: expected oid and refname")
        oid, name = parts
        match = REF.fullmatch(name)
        if not OID.fullmatch(oid) or oid == "0" * 40 or match is None:
            raise ValueError(f"cursor line {index}: invalid oid or refname")
        nid = match[1]
        try:
            if rad.parse_node_id(nid) != nid:
                raise ValueError("noncanonical node id")
        except ValueError as error:
            raise ValueError(f"cursor line {index}: invalid node id") from error
        if name <= previous:
            raise ValueError(f"cursor line {index}: unsorted or duplicate refname")
        previous = name
        tips[nid] = oid
    return tips


def emit_cursor(rid, tips):
    text = HEADER + rid + "\n" + "".join(
        f"{tips[nid]} refs/namespaces/{nid}/refs/rad/sigrefs\n" for nid in sorted(tips)
    )
    parse_cursor(text, rid)
    return text


class Reader:
    def __init__(self, repository):
        self.repo = repository
        self.stats = {"namespace_tips": 0, "signed_refs_at": 0, "commit": 0, "ref_names": 0}
        self.sets = {}
        self.parents = {}

    def tips(self):
        self.stats["namespace_tips"] += 1
        return {tip.node_id: tip.target for tip in self.repo.namespace_tips()}

    def signed(self, nid, oid):
        key = (nid, oid)  # verification is namespace-bound, not merely OID-bound
        if key not in self.sets:
            self.stats["signed_refs_at"] += 1
            self.sets[key] = self.repo.signed_refs_at(nid, oid).refs()
            self.stats["ref_names"] += len(self.sets[key])
        return self.sets[key]

    def ancestor(self, old, new):
        pending, seen = [new], set()
        while pending:
            oid = pending.pop()
            if oid == old:
                return True
            if oid in seen:
                continue
            seen.add(oid)
            if oid not in self.parents:
                self.stats["commit"] += 1
                self.parents[oid] = self.repo.commit(oid).parents
            pending.extend(self.parents[oid])
        return False


def frontier(sets):
    result = {}
    for refs in sets.values():
        for name, target in refs.items():
            parts = name.split("/")
            if len(parts) != 4 or parts[:2] != ["refs", "cobs"]:
                continue
            typename, entity = parts[2:]
            try:
                rad.parse_object_id(entity)
            except ValueError as error:
                raise rad.GitError(f"invalid signed COB ref: {name}") from error
            # Application shape parsing only: typename validation is not exposed
            # publicly and is not reimplemented here. No typed-store filtering.
            result.setdefault((typename, entity), set()).add(target)
    return result


def since(repository, cursor, *, full_scan=False):
    """Return one fully materialized page and read counters, without writing.

    full_scan disables the idle shortcut and reloads every signed set: the
    always-scan comparator for measurements. There is no cross-poll cache.
    """
    rid = repository.rid
    old = parse_cursor(cursor, rid)
    reader = Reader(repository)
    new = reader.tips()
    page = {"rid": rid, "cursor": cursor, "next": emit_cursor(rid, new),
            "namespaces": [], "incomplete": [], "cobs": []}
    if old == new and not full_scan:
        return page, reader.stats
    old_sets, new_sets = {}, {}
    # Load all current sets first: present-state failures abort, never become gaps.
    for nid, oid in sorted(new.items()):
        new_sets[nid] = reader.signed(nid, oid)
    for nid, oid in sorted(old.items()):
        try:
            old_sets[nid] = reader.signed(nid, oid)
        except rad.GitError as error:
            page["incomplete"].append(nid)
            page["namespaces"].append({"node_id": nid, "kind": "history-unavailable",
                                       "old": oid, "new": new.get(nid), "error": str(error)})
    for nid in sorted(set(old) | set(new)):
        if old.get(nid) == new.get(nid) or nid in page["incomplete"]:
            continue
        kind = ("created" if nid not in old else "removed" if nid not in new else
                "updated" if reader.ancestor(old[nid], new[nid]) else "forced")
        page["namespaces"].append({"node_id": nid, "kind": kind,
                                   "old": old.get(nid), "new": new.get(nid)})
    page["namespaces"].sort(key=lambda record: record["node_id"])
    before, after = frontier(old_sets), frontier(new_sets)
    for key in sorted(set(before) | set(after)):
        previous, current = sorted(before.get(key, set())), sorted(after.get(key, set()))
        if previous == current:
            continue
        page["cobs"].append({"typename": key[0], "id": key[1],
                             "kind": "created" if not previous else "deleted" if not current else "changed",
                             "old_tips": previous, "new_tips": current,
                             "old_complete": not page["incomplete"]})
    return page, reader.stats


def save_cursor(path, text):
    fd, temporary = tempfile.mkstemp(prefix=f".{path.name}.", dir=path.parent)
    try:
        with os.fdopen(fd, "wb") as stream:
            stream.write(text.encode("utf-8"))
            stream.flush()
            os.fsync(stream.fileno())
        os.replace(temporary, path)
    finally:
        try:
            os.unlink(temporary)
        except FileNotFoundError:
            pass


def poll_once(profile, rid, filename, handle, *, replay=False):
    if not callable(handle):
        raise TypeError("handle must be a synchronous callable")
    rid = rad.parse_repo_id(rid)
    path = checkpoint_path(profile, filename)
    repo = profile.storage.repository(rid)
    try:
        cursor = path.read_bytes().decode("utf-8")
    except FileNotFoundError:
        if not replay:
            text = emit_cursor(rid, {tip.node_id: tip.target for tip in repo.namespace_tips()})
            save_cursor(path, text)
            return None
        cursor = emit_cursor(rid, {})
    page, _ = since(repo, cursor)
    if page["namespaces"] or page["cobs"]:
        result = handle(page)
        if inspect.isawaitable(result):
            if inspect.iscoroutine(result):
                result.close()
            raise TypeError("handle must be synchronous")
    save_cursor(path, page["next"])
    return page


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("rid")
    parser.add_argument("--state", required=True)
    parser.add_argument("--replay", action="store_true")
    args = parser.parse_args()

    def output(page):
        print(json.dumps(page, sort_keys=True, ensure_ascii=True))
        sys.stdout.flush()

    poll_once(rad.Profile.load(), args.rid, args.state, output, replay=args.replay)


if __name__ == "__main__":
    main()
