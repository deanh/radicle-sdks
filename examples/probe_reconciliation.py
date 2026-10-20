"""Destructive retention probes on temporary COPIES of a local storage repository.

Never run Git writes against the source. Requires a quiescent source; use only
on this experiment's supported local Linux setup. Not an SDK or backup tool.
"""

import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import time

import radiclepy as rad

from reconcile_refs import emit_cursor, since


def git(path, *args):
    return subprocess.check_output(["git", "-C", str(path), *args], text=True).strip()


def scan_oracle(repository, cursor):
    """Independent full signed-set scan, not the experiment's frontier reducer."""
    from reconcile_refs import parse_cursor

    current = {tip.node_id: tip.target for tip in repository.namespace_tips()}
    advertised = []
    for remote in repository.remotes():
        refs = repository.signed_refs_at(remote.fetcher, current[remote.fetcher]).refs()
        advertised.extend((name, refs[name]) for name in remote.heads)
    previous, incomplete = [], []
    for nid, oid in parse_cursor(cursor, repository.rid).items():
        try:
            previous.extend(repository.signed_refs_at(nid, oid).refs().items())
        except rad.GitError:
            incomplete.append(nid)
    before, after = {}, {}
    for entries, destination in [(previous, before), (advertised, after)]:
        for name, target in entries:
            if name.startswith("refs/cobs/"):
                _, _, typename, entity = name.split("/")
                destination.setdefault((typename, entity), set()).add(target)
    records = []
    for key in sorted(set(before) | set(after)):
        old, new = sorted(before.get(key, set())), sorted(after.get(key, set()))
        if old != new:
            records.append({"typename": key[0], "id": key[1], "old_tips": old,
                "new_tips": new, "old_complete": not incomplete,
                "kind": "created" if not old else "deleted" if not new else "changed"})
    return records, sorted(incomplete)


def omit_object(repository_path, oid, scratch):
    """Really remove a packed object in a disposable copy, not a loose shadow."""
    objects = git(repository_path, "cat-file", "--batch-all-objects", "--batch-check=%(objectname)").splitlines()
    scratch.mkdir()
    subprocess.run(["git", "-C", str(repository_path), "pack-objects", str(scratch / "pack")],
        input="\n".join(o for o in objects if o != oid) + "\n",
        text=True, capture_output=True, check=True)
    shutil.rmtree(repository_path / "objects")
    (repository_path / "objects" / "pack").mkdir(parents=True)
    for file in scratch.iterdir():
        shutil.copy(file, repository_path / "objects" / "pack" / file.name)


def probe(profile, rid, *, node_script=None):
    source = Path(profile.storage.path) / rad.parse_repo_id(rid).removeprefix("rad:")
    original = git(source, "for-each-ref", "--format=%(refname) %(objectname)")
    rows = []
    for scenario in ["removed", "forced", "pruned", "current-unavailable", "ancestry-unavailable"]:
        with tempfile.TemporaryDirectory(prefix="reconciliation-copy-") as temporary:
            root = Path(temporary)
            copied_profile = rad.Profile.init(str(root / "home"), "retention-copy")
            target = Path(copied_profile.storage.path) / rid.removeprefix("rad:")
            # Do not copy active/stale lock files or use hardlinks/alternates.
            shutil.copytree(source, target, ignore=shutil.ignore_patterns("*.lock"))
            repository = copied_profile.storage.repository(rid)
            tips = {tip.node_id: tip.target for tip in repository.namespace_tips()}
            candidates = [(nid, repository.signed_refs_at(nid, oid).parent) for nid, oid in sorted(tips.items())]
            nid, parent = next((nid, parent) for nid, parent in candidates if parent)
            current = tips[nid]
            cursor = emit_cursor(rid, tips)
            sigref = f"refs/namespaces/{nid}/refs/rad/sigrefs"
            if scenario == "removed":
                git(target, "update-ref", "-d", sigref)
            elif scenario in ("forced", "pruned"):
                git(target, "update-ref", sigref, parent)
                if scenario == "pruned":
                    git(target, "reflog", "expire", "--expire=now", "--all")
                    git(target, "gc", "--prune=now")
            elif scenario == "current-unavailable":
                (target / sigref).parent.mkdir(parents=True, exist_ok=True)
                (target / sigref).write_text("1" * 40 + "\n")
            else:
                grandparent = repository.signed_refs_at(nid, parent).parent
                if grandparent is None:
                    raise ValueError("ancestry probe needs at least three sigrefs commits")
                tips[nid] = grandparent
                cursor = emit_cursor(rid, tips)
                omit_object(target, parent, root / "repacked")
            start = time.perf_counter()
            try:
                page, stats = since(repository, cursor)
                elapsed = (time.perf_counter() - start) * 1000
                expected_cobs, expected_gaps = scan_oracle(repository, cursor)
                if page["cobs"] != expected_cobs or page["incomplete"] != expected_gaps:
                    raise AssertionError("signed-set scan oracle disagrees")
                outcome = {"namespaces": page["namespaces"], "incomplete": page["incomplete"],
                    "cobs": len(page["cobs"]), "cobKinds": sorted({c["kind"] for c in page["cobs"]}),
                    "oldComplete": all(c["old_complete"] for c in page["cobs"]), "stats": stats}
            except rad.GitError as error:
                elapsed = (time.perf_counter() - start) * 1000
                outcome = {"error": "GitError", "message": str(error)}
            row = {"scenario": scenario, "outcome": outcome, "pythonMs": elapsed}
            if node_script:
                code = """
import {Profile} from 'radicle-sdk';
const [script,rid,cursor] = process.argv.slice(1);
const {since} = await import(script);
try { console.log(JSON.stringify(await since(Profile.load().storage.repository(rid),cursor))); }
catch(e) { console.log(JSON.stringify({error:e.code,message:e.message})); }
"""
                start = time.perf_counter()
                node = json.loads(subprocess.check_output(
                    ["node", "--input-type=module", "-e", code, Path(node_script).resolve().as_uri(), rid, cursor],
                    cwd=Path(node_script).resolve().parent,
                    env=dict(os.environ, RAD_HOME=copied_profile.home.path), text=True))
                row["nodeProcessMs"] = (time.perf_counter() - start) * 1000
                if "error" in outcome:
                    if node.get("error") != "GIT":
                        raise AssertionError(node)
                elif node != {"page": page, "stats": stats}:
                    raise AssertionError("cross-SDK retention results disagree")
            expected = {"removed": "removed", "forced": "forced", "pruned": "history-unavailable"}
            if scenario in expected:
                if outcome.get("namespaces", [{}])[0].get("kind") != expected[scenario]:
                    raise AssertionError(f"{scenario}: unexpected outcome {outcome}")
            elif outcome.get("error") != "GitError":
                raise AssertionError(f"{scenario}: expected GitError")
            rows.append(row)
    if original != git(source, "for-each-ref", "--format=%(refname) %(objectname)"):
        raise RuntimeError("source refs changed during the experiment; results invalid")
    return rows


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("rid")
    parser.add_argument("--node-script", help="optional installed reconcile-refs.mjs for conformance")
    args = parser.parse_args()
    print(json.dumps(probe(rad.Profile.load(), args.rid, node_script=args.node_script), indent=2))


if __name__ == "__main__":
    main()
