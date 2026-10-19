"""Operation-feed experiment, not an SDK API. See events_experiment.md."""

import argparse
import base64
import inspect
import json
from pathlib import Path
import sys
import time

import radiclepy as rad
from watch_issues import checkpoint_path, save_checkpoint

TYPES = ("xyz.radicle.issue", "xyz.radicle.patch")
SAFE = 9007199254740991


class ResyncRequired(RuntimeError):
    """The compact frontier cannot safely establish the previously seen history."""


def preview(value):
    # This JSON wire preview is not authoritative numeric/lexical serialization.
    # Exact original bytes are always included in rawActions.
    if type(value) is int and abs(value) > SAFE:
        return str(value)
    if isinstance(value, dict):
        return {key: preview(item) for key, item in value.items()}
    if isinstance(value, (list, tuple)):
        return [preview(item) for item in value]
    return value


def event_for(rid, typename, entity, op):
    domain = "issue" if typename == TYPES[0] else "patch"
    try:
        payload = preview(op.actions_json())
        action_types = [a.get("type") if isinstance(a, dict)
                        and isinstance(a.get("type"), str) else None for a in payload]
        decode_error = False
    except rad.CobError:
        payload, action_types, decode_error = None, [], True
    kinds = [f"{domain}.{kind}Recorded" if kind else f"{domain}.unknownAction"
             for kind in action_types]
    return {
        "id": op.id, "rid": rid, "cob": {"type": typename, "id": entity},
        "kind": f"{domain}.opened" if op.id == entity else (
            kinds[0] if len(kinds) == 1 else f"{domain}.operation"
        ),
        "actor": rad.parse_did(f"did:key:{op.actor}"),
        "timestamp": str(op.timestamp), "parents": list(op.parents),
        "actionKinds": kinds, "payload": payload,
        "rawActions": [base64.b64encode(a).decode("ascii") for a in op.actions],
        "decodeError": decode_error, "evaluation": "not-evaluated",
    }


class Reader:
    def __init__(self, repo):
        self.repo, self.raw = repo, rad.Raw.open(repo)
        self.calls = {}
        self.history_entries = self.sigrefs_bytes = self.discovery_refs = 0
        self.blobs = {}

    def call(self, name, *args):
        self.calls[name] = self.calls.get(name, 0) + 1
        owner, method = name.split(".")
        return getattr(self.raw if owner == "raw" else self.repo, method)(*args)

    def scan(self):
        result = {}
        for typename in TYPES:
            for entity in self.call("raw.entities", typename):
                tips = sorted({r.target for r in self.call("raw.refs", typename, entity)})
                if tips:
                    result[f"{typename}/{entity}"] = tips
        return result

    def signatures(self):
        # Public remotes() discovers namespaces, but also loads their ref sets.
        # Counting this as one call does NOT make it O(namespaces) backend work.
        remotes = self.call("repo.remotes")
        self.discovery_refs = sum(len(remote.heads) for remote in remotes)
        return {
            remote.fetcher: self.call(
                "repo.rev_parse", f"refs/namespaces/{remote.fetcher}/refs/rad/sigrefs"
            )
            for remote in remotes
        }

    def signed_frontier(self, signatures):
        result = {}
        for oid in signatures.values():
            if oid not in self.blobs:
                commit = self.call("repo.commit", oid)
                tree = self.call("repo.tree", commit.tree)
                entry = next((e for e in tree.entries if e.name_str == "refs"), None)
                if entry is None or entry.kind != "blob":
                    raise ResyncRequired("sigrefs refs blob missing")
                data = self.call("repo.blob", entry.oid)
                self.sigrefs_bytes += len(data)
                refs = {}
                for line in data.decode("utf-8").splitlines():
                    target, name = line.split()
                    target = rad.parse_oid(target)
                    parts = name.split("/")
                    if len(parts) == 4 and parts[:2] == ["refs", "cobs"] and parts[2] in TYPES:
                        try:
                            entity = rad.parse_object_id(parts[3])
                        except ValueError:
                            continue
                        refs[f"{parts[2]}/{entity}"] = target
                self.blobs[oid] = refs
            for key, target in self.blobs[oid].items():
                result.setdefault(key, set()).add(target)
        return {key: sorted(tips) for key, tips in result.items()}


def load_cursor(path, storage, rid, source):
    try:
        cursor = json.loads(path.read_text(encoding="utf-8"))
    except FileNotFoundError:
        return None
    if (not isinstance(cursor, dict)
            or set(cursor) != {"version", "model", "source", "storage", "rid", "frontier"}
            or cursor["version"] != 1 or isinstance(cursor["version"], bool)
            or cursor["model"] != "operations" or cursor["source"] != source
            or cursor["storage"] != storage or cursor["rid"] != rid
            or not isinstance(cursor["frontier"], dict)):
        raise ValueError("invalid operation cursor version/model/source/scope/shape")
    for key, value in cursor["frontier"].items():
        if source == "sigrefs":
            if rad.parse_node_id(key) != key or not isinstance(value, str) or rad.parse_oid(value) != value:
                raise ValueError("invalid sigrefs cursor")
        else:
            parts = key.split("/")
            if (len(parts) != 2 or parts[0] not in TYPES
                    or rad.parse_object_id(parts[1]) != parts[1]
                    or not isinstance(value, list) or not value
                    or any(not isinstance(v, str) or rad.parse_oid(v) != v for v in value)
                    or len(set(value)) != len(value)):
                raise ValueError("invalid ref frontier cursor")
    return cursor


def reachable(tips, ops):
    seen, stack = set(), list(tips)
    while stack:
        oid = stack.pop()
        if oid in seen:
            continue
        if oid not in ops:
            raise ResyncRequired("frontier tip/ancestor unavailable through current Raw.history; cursor retained")
        seen.add(oid)
        stack.extend(ops[oid].parents)
    return seen


def poll_operations(profile, rid, filename, handle, *, source="scan", replay=False):
    """One op per event, all actions retained; cursor saved only after delivery.

    Non-fast-forward/removal/history-unavailable cases fail closed, not lossless
    recovery. The SDK currently cannot load old arbitrary COB entries publicly.
    """
    if source not in ("scan", "sigrefs") or not callable(handle):
        raise ValueError("require scan/sigrefs source and synchronous callable")
    started = time.perf_counter()
    rid = rad.parse_repo_id(rid)
    path = checkpoint_path(profile, filename)
    storage = str(Path(profile.storage.path).resolve(strict=True))
    previous = load_cursor(path, storage, rid, source)
    reader = Reader(profile.storage.repository(rid))
    frontier = reader.scan() if source == "scan" else reader.signatures()
    cursor = {"version": 1, "model": "operations", "source": source,
              "storage": storage, "rid": rid, "frontier": frontier}
    events = []
    if previous is not None and previous["frontier"] == frontier:
        pass
    elif previous is None and not replay:
        pass  # Baseline needs refs only, no typed evaluation or history walk.
    else:
        old = {} if previous is None else previous["frontier"]
        if source == "sigrefs":
            old = reader.signed_frontier(old)
            new = reader.signed_frontier(frontier)
        else:
            new = frontier
        for key in sorted(set(old) | set(new)):
            if old.get(key, []) == new.get(key, []):
                continue
            if key not in new:
                raise ResyncRequired("COB refs removed; cursor retained, choose an explicit recovery policy")
            typename, entity = key.split("/")
            history = reader.call("raw.history", typename, entity)
            reader.history_entries += len(history)
            ops = {op.id: op for op in history}
            before = reachable(old.get(key, []), ops)
            after = reachable(new[key], ops)
            if not before <= after:
                raise ResyncRequired("non-fast-forward frontier; cursor retained")
            events.extend(event_for(rid, typename, entity, op)
                          for op in history if op.id in after - before)
    # Every read, graph check and classification precedes application callbacks.
    for event in events:
        result = handle(event)
        if inspect.isawaitable(result):
            if inspect.iscoroutine(result):
                result.close()
            raise TypeError("synchronous callback required")
    if cursor != previous:
        save_checkpoint(path, cursor)
    return {"events": len(events), "calls": reader.calls,
            "historyEntries": reader.history_entries, "sigrefsBytes": reader.sigrefs_bytes,
            "discoveryRefs": reader.discovery_refs,
            "cursorBytes": len(json.dumps(cursor, separators=(",", ":")).encode()),
            "seconds": time.perf_counter() - started}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("rid")
    parser.add_argument("--state", required=True)
    parser.add_argument("--source", choices=("scan", "sigrefs"), default="scan")
    parser.add_argument("--replay", action="store_true")
    args = parser.parse_args()
    stats = poll_operations(rad.Profile.load(), args.rid, args.state,
                            lambda event: print(json.dumps(event, sort_keys=True), flush=True),
                            source=args.source, replay=args.replay)
    print(json.dumps(stats, sort_keys=True), file=sys.stderr)


if __name__ == "__main__":
    try:
        main()
    except Exception as error:
        print(f"operation experiment failed: {error}", file=sys.stderr)
        sys.exit(1)
