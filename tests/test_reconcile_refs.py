"""Reconciliation experiment: real storage, signed-set oracle and CLI handoff."""

import importlib.util
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
from unittest.mock import patch

import pytest
import radiclepy as rad

from sdk_fixture import git, project_fixture

ROOT = Path(__file__).resolve().parent.parent


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


with patch.dict(sys.modules, {"watch_issues": load("watch_issues", ROOT / "examples/watch_issues.py")}):
    feed = load("reconcile_refs", ROOT / "examples/reconcile_refs.py")


def require_node():
    if shutil.which("node") is None or not (ROOT / "node/dist/index.js").exists():
        if os.environ.get("RADICLE_REQUIRE_CROSS_SDK") == "1":
            pytest.fail("required Node SDK unavailable")
        pytest.skip("Node SDK unavailable")


def node_since(profile, rid, cursor, *, full=False):
    require_node()
    code = """
import { Profile } from 'radicle-sdk';
import { since } from './examples/reconcile-refs.mjs';
const [rid, cursor, full] = process.argv.slice(1);
const result = await since(Profile.load().storage.repository(rid), cursor, { fullScan: full === 'yes' });
console.log(JSON.stringify(result));
"""
    result = subprocess.run(["node", "--input-type=module", "-e", code, rid, cursor, "yes" if full else "no"],
        cwd=ROOT, env=dict(os.environ, RAD_HOME=profile.home.path), capture_output=True, text=True, timeout=90)
    assert result.returncode == 0, result.stderr
    return json.loads(result.stdout)


def cursor(repo):
    return feed.emit_cursor(repo.rid, {t.node_id: t.target for t in repo.namespace_tips()})


def oracle(repo):
    """Always rescan signed sets, independent of the experiment's frontier code."""
    tips = {t.node_id: t.target for t in repo.namespace_tips()}
    result = {}
    for remote in repo.remotes():
        refs = repo.signed_refs_at(remote.fetcher, tips[remote.fetcher]).refs()
        for name in remote.heads:
            if name.startswith("refs/cobs/"):
                _, _, typename, entity = name.split("/")
                result.setdefault((typename, entity), set()).add(refs[name])
    return result


def assert_delta(page, before, after):
    expected = []
    for key in sorted(set(before) | set(after)):
        old, new = sorted(before.get(key, set())), sorted(after.get(key, set()))
        if old != new:
            expected.append({"typename": key[0], "id": key[1], "old_tips": old, "new_tips": new,
                "kind": "created" if not old else "deleted" if not new else "changed", "old_complete": True})
    assert page["cobs"] == expected


def test_reconciliation_oracle_cross_sdk_downtime_and_idle(tmp_path):
    profile, peer, _, repo, _, stored, base = project_fixture(tmp_path)
    issue = rad.Issues.open_as(repo, profile.signer()).create("issue", "root")
    patch_handle = rad.Patches.open_as(repo, profile.signer()).create("patch", "root", base=base, head=base)
    start, before = cursor(repo), oracle(repo)
    idle, stats = feed.since(repo, start)
    assert not idle["namespaces"] and not idle["cobs"]
    assert stats == {"namespace_tips": 1, "signed_refs_at": 0, "commit": 0, "ref_names": 0}
    scanned, scan_stats = feed.since(repo, start, full_scan=True)
    assert idle == scanned
    assert scan_stats["signed_refs_at"] > 0
    assert node_since(profile, repo.rid, start) == {"page": idle, "stats": stats}
    # Several offline operations collapse to one changed COB trigger, not actions.
    issue.edit("second")
    issue.comment("offline one")
    issue.comment("offline two")
    rad.Patches.open_as(repo, peer.signer()).get_mut(patch_handle.id).review(
        patch_handle.patch.latest_revision, "accept", "review")
    after = oracle(repo)
    refs_before = git(stored, "for-each-ref", "--format=%(refname) %(objectname)")
    page, stats = feed.since(repo, start)
    assert_delta(page, before, after)
    assert {r["kind"] for r in page["cobs"]} == {"changed"}
    assert node_since(profile, repo.rid, start) == {"page": page, "stats": stats}
    assert feed.since(repo, start, full_scan=True)[0] == page
    assert refs_before == git(stored, "for-each-ref", "--format=%(refname) %(objectname)")
    assert feed.since(repo, page["next"])[0]["cobs"] == []
    replay, _ = feed.since(repo, feed.emit_cursor(repo.rid, {}))
    assert_delta(replay, {}, after)
    assert node_since(profile, repo.rid, replay["cursor"])["page"] == replay


def test_reconciliation_union_removal_rewind_restore_and_unsigned_window(tmp_path):
    profile, peer, _, repo, _, stored, _ = project_fixture(tmp_path)
    issue = rad.Issues.open_as(repo, profile.signer()).create("union", "root")
    peer_issue = rad.Issues.open_as(repo, peer.signer()).get_mut(issue.id)
    peer_tip = peer_issue.comment("peer")
    local_tip = issue.comment("local")
    start, before = cursor(repo), oracle(repo)
    visible = [comment.id for comment in rad.Issues.open(repo).get(issue.id).comments]
    peer_ref = f"refs/namespaces/{peer.node_id}/refs/cobs/xyz.radicle.issue/{issue.id}"
    git(stored, "update-ref", "-d", peer_ref)
    repo.sign_refs(peer.signer())
    narrowed, _ = feed.since(repo, start)
    assert_delta(narrowed, before, oracle(repo))
    assert narrowed["cobs"][0]["kind"] == "changed"
    assert [comment.id for comment in rad.Issues.open(repo).get(issue.id).comments] == visible
    assert node_since(profile, repo.rid, start)["page"] == narrowed
    # A COB rewind is changed, regardless of causal ancestry; namespace signing
    # still advances normally and is an updated namespace, not forced.
    local_ref = f"refs/namespaces/{profile.node_id}/refs/cobs/xyz.radicle.issue/{issue.id}"
    before = oracle(repo)
    git(stored, "update-ref", local_ref, issue.id)
    repo.sign_refs(profile.signer())
    rewind, _ = feed.since(repo, narrowed["next"])
    assert_delta(rewind, before, oracle(repo))
    assert rewind["cobs"][0]["kind"] == "changed"
    # Changing raw refs without signing does not change this feed.
    signed_cursor = cursor(repo)
    git(stored, "update-ref", local_ref, local_tip)
    assert feed.since(repo, signed_cursor)[0]["cobs"] == []
    repo.sign_refs(profile.signer())
    restored, _ = feed.since(repo, signed_cursor)
    assert restored["cobs"][0]["new_tips"] == [local_tip]
    git(stored, "update-ref", "-d", local_ref)
    repo.sign_refs(profile.signer())
    deleted, _ = feed.since(repo, restored["next"])
    assert deleted["cobs"][0]["kind"] == "deleted"
    assert node_since(profile, repo.rid, restored["next"])["page"] == deleted
    assert peer_tip != local_tip


def test_reconciliation_namespace_forced_removed_pruned_and_current_error(tmp_path):
    profile, _, _, repo, _, stored, _ = project_fixture(tmp_path)
    initial = cursor(repo)
    issue = rad.Issues.open_as(repo, profile.signer()).create("retained", "root")
    advanced = cursor(repo)
    sigref = f"refs/namespaces/{profile.node_id}/refs/rad/sigrefs"
    old_oid = feed.parse_cursor(initial, repo.rid)[profile.node_id]
    git(stored, "update-ref", sigref, old_oid)
    forced, _ = feed.since(repo, advanced)
    assert forced["namespaces"][0]["kind"] == "forced"
    assert forced["cobs"][0]["kind"] == "deleted"
    assert node_since(profile, repo.rid, advanced)["page"] == forced
    git(stored, "update-ref", "-d", sigref)
    removed, _ = feed.since(repo, advanced)
    assert removed["namespaces"][0]["kind"] == "removed"
    assert removed["cobs"][0]["kind"] == "deleted"
    # Recreate the old tip and collect its orphaned newer sigrefs commit.
    git(stored, "update-ref", sigref, old_oid)
    git(stored, "reflog", "expire", "--expire=now", "--all")
    git(stored, "gc", "--prune=now")
    gap, _ = feed.since(repo, advanced)
    assert gap["incomplete"] == [profile.node_id]
    assert gap["namespaces"][0]["kind"] == "history-unavailable"
    assert node_since(profile, repo.rid, advanced)["page"] == gap
    # A missing current object never becomes a history-unavailable page.
    (Path(stored) / sigref).write_text("1" * 40 + "\n")
    with pytest.raises(rad.GitError):
        feed.since(repo, initial)
    # issue operation objects may still exist; this feed never walks them.
    assert issue.id


def test_reconciliation_incomplete_old_frontiers_are_explicit(tmp_path):
    profile, peer, _, repo, _, stored, _ = project_fixture(tmp_path)
    issue = rad.Issues.open_as(repo, profile.signer()).create("gap", "root")
    repo.sign_refs(peer.signer())
    tips = feed.parse_cursor(cursor(repo), repo.rid)
    tips[profile.node_id] = "1" * 40
    broken_old = feed.emit_cursor(repo.rid, tips)
    page, _ = feed.since(repo, broken_old)
    assert page["incomplete"] == [profile.node_id]
    record = next(record for record in page["cobs"] if record["id"] == issue.id)
    assert record["kind"] == "created"
    assert record["old_complete"] is False
    assert all(not record["old_complete"] for record in page["cobs"])
    assert node_since(profile, repo.rid, broken_old)["page"] == page


def test_cursor_strictness_and_canonical_cross_sdk_bytes(tmp_path):
    profile, peer, _, repo, _, _, _ = project_fixture(tmp_path)
    tips = {profile.node_id: "1" * 40, peer.node_id: "2" * 40}
    text = feed.emit_cursor(repo.rid, tips)
    assert feed.parse_cursor(text, repo.rid) == tips
    assert feed.emit_cursor(repo.rid, dict(reversed(list(tips.items())))) == text
    lines = text.splitlines(keepends=True)
    invalid = [text[:-1], text + "\n", text + "garbage", "\ufeff" + text,
        text.replace("v1", "v2"), text.replace("\n", "\r\n"),
        text.replace(repo.rid, "rad:z111"), lines[0] + lines[2] + lines[1],
        text + lines[1], text.replace("1" * 40, "0" * 40),
        text.replace("1" * 40, "a" * 39 + "A"), text.replace("1" * 40, "1" * 39),
        text.replace("1" * 40, "1" * 41), text.replace("sigrefs", "sigrefs "),
        text.replace("sigrefs", "sigrefs\r"), text.replace("1" * 40, "1" * 40 + "\r"),
        text.replace(profile.node_id, "bad-node"), text.replace("/sigrefs", "/other")]
    samples = []
    for i in range(1, 33):
        mapping = {} if i % 3 == 0 else {profile.node_id: f"{i:040x}", peer.node_id: f"{i + 32:040x}"}
        sample = feed.emit_cursor(repo.rid, mapping)
        assert feed.parse_cursor(sample, repo.rid) == mapping
        samples.append(sample)
    for bad in invalid:
        with pytest.raises(ValueError, match="cursor line"):
            feed.parse_cursor(bad, repo.rid)
    require_node()
    code = """
import assert from 'node:assert/strict';
import { parseCursor, emitCursor } from './examples/reconcile-refs.mjs';
const [rid, text, invalid, samples] = process.argv.slice(1);
assert.equal(emitCursor(rid, parseCursor(text,rid)), text);
for (const sample of JSON.parse(samples)) assert.equal(emitCursor(rid,parseCursor(sample,rid)),sample);
for (const bad of JSON.parse(invalid)) assert.throws(()=>parseCursor(bad,rid), /cursor line/);
"""
    result = subprocess.run(["node", "--input-type=module", "-e", code, repo.rid, text, json.dumps(invalid), json.dumps(samples)],
                            cwd=ROOT, text=True, capture_output=True)
    assert result.returncode == 0, result.stderr


def test_checkpoint_cli_handoff_callback_and_save_failure(tmp_path):
    profile, _, _, repo, _, stored, _ = project_fixture(tmp_path / "project")
    state = tmp_path / "cursor"
    assert feed.poll_once(profile, repo.rid, state, lambda page: pytest.fail("baseline")) is None
    saved = state.read_bytes()
    issue = rad.Issues.open_as(repo, profile.signer()).create("cli", "root")
    observed = []

    def fail(page):
        observed.append(page)
        raise RuntimeError("handler")

    with pytest.raises(RuntimeError, match="handler"):
        feed.poll_once(profile, repo.rid, state, fail)
    assert state.read_bytes() == saved

    async def asynchronous(page):
        pytest.fail("async callback must not run")

    with pytest.raises(TypeError, match="synchronous"):
        feed.poll_once(profile, repo.rid, state, asynchronous)
    assert state.read_bytes() == saved
    require_node()
    result = subprocess.run(["node", str(ROOT / "examples/reconcile-refs.mjs"), "--state", str(state), repo.rid],
        cwd=tmp_path, env=dict(os.environ, RAD_HOME=profile.home.path), text=True, capture_output=True)
    assert result.returncode == 0, result.stderr
    assert json.loads(result.stdout) == observed[0]
    assert state.read_bytes() == observed[0]["next"].encode()
    issue.comment("python resumes node")
    before = git(stored, "for-each-ref", "--format=%(refname) %(objectname)")
    next_saved = state.read_bytes()
    with patch.object(feed.os, "replace", side_effect=OSError("replace")):
        with pytest.raises(OSError, match="replace"):
            feed.poll_once(profile, repo.rid, state, observed.append)
    assert state.read_bytes() == next_saved
    assert not list(tmp_path.glob(".cursor.*"))
    page = feed.poll_once(profile, repo.rid, state, observed.append)
    assert page == observed[-2]
    assert before == git(stored, "for-each-ref", "--format=%(refname) %(objectname)")
    state.write_bytes(b"invalid")
    with pytest.raises(ValueError):
        feed.poll_once(profile, repo.rid, state, observed.append)
    assert state.read_bytes() == b"invalid"
    replay_state = tmp_path / "replay.cursor"
    result = subprocess.run([sys.executable, str(ROOT / "examples/reconcile_refs.py"),
        repo.rid, "--replay", "--state", str(replay_state)], cwd=tmp_path,
        env=dict(os.environ, RAD_HOME=profile.home.path), text=True, capture_output=True)
    assert result.returncode == 0, result.stderr
    assert json.loads(result.stdout) == feed.since(repo, feed.emit_cursor(repo.rid, {}))[0]
    assert replay_state.read_bytes() == cursor(repo).encode()


def test_missing_intermediate_sigrefs_commit_is_a_poll_error(tmp_path):
    profile, _, _, repo, _, stored, _ = project_fixture(tmp_path)
    old = cursor(repo)
    issue = rad.Issues.open_as(repo, profile.signer()).create("ancestry", "root")
    intermediate = repo.namespace_tips()[0].target
    issue.comment("newer")
    # These tiny fixtures have loose objects; genuinely remove the intermediate
    # commit while leaving both endpoint signed sets readable.
    (Path(stored) / "objects" / intermediate[:2] / intermediate[2:]).unlink()
    with pytest.raises(rad.GitError):
        feed.since(repo, old)
    require_node()
    code = """
import assert from 'node:assert/strict';
import { Profile } from 'radicle-sdk';
import { since } from './examples/reconcile-refs.mjs';
await assert.rejects(since(Profile.load().storage.repository(process.argv[1]), process.argv[2]), e=>e.code==='GIT');
"""
    result = subprocess.run(["node", "--input-type=module", "-e", code, repo.rid, old],
        cwd=ROOT, env=dict(os.environ, RAD_HOME=profile.home.path), text=True, capture_output=True)
    assert result.returncode == 0, result.stderr
