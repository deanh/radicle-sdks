"""Cross-language operation events and compact sigrefs-cursor handoff."""

import asyncio
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile

import pytest
import radiclepy as rad

from sdk_fixture import git, project_fixture

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / "examples"))
import operation_events as python_feed

NODE_SCRIPT = ROOT / "examples/operation-events.mjs"

NODE_MAIN = """
import {{ pollOperations }} from {script};
const {{ Profile }} = await import('radicle-sdk');
const [rid, replay, path, source] = process.argv.slice(1);
const options = {{ source: source, replay: replay === 'yes' }};
const events = [];
const stats = await pollOperations(Profile.load(), rid, path, (e) => events.push(e), options);
process.stdout.write(
  JSON.stringify(
    {{ events: events, stats: stats }},
    (key, value) => (typeof value === 'bigint' ? value.toString() : value),
  ),
);
"""


def node_poll(profile, rid, path, source, replay=False):
    if shutil.which("node") is None or not (ROOT / "node/dist/index.js").exists():
        if os.environ.get("RADICLE_REQUIRE_CROSS_SDK") == "1":
            pytest.fail("required Node SDK unavailable")
        pytest.skip("Node SDK unavailable")
    result = subprocess.run(
        ["node", "--input-type=module", "-e", NODE_MAIN.format(script=json.dumps(str(NODE_SCRIPT))),
         rid, "yes" if replay else "no", str(path), source],
        cwd=ROOT, env=dict(os.environ, RAD_HOME=profile.home.path),
        text=True, capture_output=True, timeout=90,
    )
    assert result.returncode == 0, result.stderr
    return json.loads(result.stdout)


def normalize(events):
    """Compare the language-neutral contract, not preview JSON encodings."""
    result = json.loads(json.dumps(events))
    for event in result:
        event.pop("payload", None)  # numeric preview encodings intentionally differ
    return result


def python_events(profile, rid, source, replay=True, path=None):
    own = path is None
    if own:
        handle = tempfile.NamedTemporaryFile(suffix=".json", delete=False)
        handle.close()
        path = Path(handle.name)
    events = []
    python_feed.poll_operations(profile, rid, path, events.append, source=source, replay=replay)
    if own:
        path.unlink()
    return events


def fresh_python_events(profile, rid, source):
    """Replay all operations from a missing cursor (no file left behind)."""
    with tempfile.TemporaryDirectory() as temporary:
        path = Path(temporary) / "cursor.json"
        events = []
        python_feed.poll_operations(profile, rid, path, events.append, source=source, replay=True)
        return events


def test_operation_events_agree_and_share_cursors_across_languages(tmp_path):
    profile, peer, _, repo, _, stored, base = project_fixture(tmp_path / "project")
    issues = rad.Issues.open_as(repo, profile.signer())
    issue = issues.create("first", "root", labels=["a"])
    patch = rad.Patches.open_as(repo, profile.signer()).create("p", "root", base=base, head=base)
    cursors = {source: tmp_path / f"{source}.json" for source in ("scan", "sigrefs")}
    for source in ("scan", "sigrefs"):
        python_feed.poll_operations(profile, repo.rid, cursors[source], lambda event: None,
                                    source=source, replay=True)
    before = git(stored, "for-each-ref", "--format=%(refname) %(objectname)")
    for source in ("scan", "sigrefs"):
        idle = node_poll(profile, repo.rid, cursors[source], source)
        assert idle["events"] == []
        assert idle["stats"]["historyEntries"] == 0
    edits = [issue.edit("second"), issue.edit("third")]
    review = rad.Patches.open_as(repo, peer.signer()).get_mut(patch.id).review(
        patch.patch.latest_revision, "accept", "review")
    python_scan = python_events(profile, repo.rid, "scan", path=cursors["scan"], replay=False)
    python_signed = python_events(profile, repo.rid, "sigrefs", path=cursors["sigrefs"], replay=False)
    assert normalize(python_scan) == normalize(python_signed)
    assert sorted(event["id"] for event in python_scan) == sorted([*edits, review])
    after = git(stored, "for-each-ref", "--format=%(refname) %(objectname)")
    node_signed = node_poll(profile, repo.rid, cursors["sigrefs"], "sigrefs")
    assert node_signed["events"] == []
    # Signed COB writes legitimately move refs since `before` was captured.
    replayed = node_poll(profile, repo.rid, tmp_path / "node-sigrefs.json", "sigrefs", replay=True)
    assert normalize(replayed["events"]) == normalize(fresh_python_events(profile, repo.rid, "sigrefs"))
    node_replay = node_poll(profile, repo.rid, tmp_path / "node-scan.json", "scan", replay=True)
    assert normalize(node_replay["events"]) == normalize(fresh_python_events(profile, repo.rid, "scan"))
    # An unsigned new operation is invisible to the signed source until signing.
    sigref = f"refs/namespaces/{profile.node_id}/refs/rad/sigrefs"
    old = git(stored, "rev-parse", sigref)
    comment = issue.comment("later")
    git(stored, "update-ref", sigref, old)
    seen = []
    python_feed.poll_operations(profile, repo.rid, cursors["sigrefs"], seen.append, source="sigrefs")
    assert seen == []
    unsigned = node_poll(profile, repo.rid, cursors["sigrefs"], "sigrefs")
    assert unsigned["events"] == []
    # Re-signing the unchanged signed set does not advertise the newer comment;
    # a subsequent signed COB write makes it visible.
    git(stored, "update-ref", sigref, old)
    repo.sign_refs(profile.signer())
    follow = issue.comment("after re-sign")
    delivered = node_poll(profile, repo.rid, cursors["sigrefs"], "sigrefs")
    assert [event["id"] for event in delivered["events"]] == [comment, follow]
    assert node_poll(profile, repo.rid, cursors["sigrefs"], "sigrefs")["events"] == []
    # The scan source observed the unsigned change throughout.
