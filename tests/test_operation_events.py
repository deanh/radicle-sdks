"""Experiment results: operations vs evaluated state and compact frontier limits."""

import json
from pathlib import Path
import sys

import pytest
import radiclepy as rad

from sdk_fixture import git, project_fixture

sys.path.insert(0, str(Path(__file__).resolve().parent.parent / "examples"))
import operation_events as feed
import watch_issues as observer


@pytest.fixture
def data(tmp_path):
    p, peer, _, repo, _, stored, base = project_fixture(tmp_path / "project")
    return p, peer, repo, stored, base, tmp_path


def polls(data, *, replay=False):
    p, _, repo, _, _, root = data
    results = []
    for source in ("scan", "sigrefs"):
        events = []
        stats = feed.poll_operations(p, repo.rid, root / f"{source}.json", events.append,
                                     source=source, replay=replay)
        results.append((events, stats))
    return results


def test_signed_sources_agree_multiaction_replay_idle_and_offline_resume(data):
    p, _, repo, stored, base, root = data
    issue = rad.Issues.open_as(repo, p.signer()).create("first", "root", labels=["a"])
    patch = rad.Patches.open_as(repo, p.signer()).create("patch", "root", base=base, head=base)
    before = git(stored, "for-each-ref", "--format=%(refname) %(objectname)")
    scan, signed = polls(data, replay=True)
    assert scan[0] == signed[0]
    assert {event["id"] for event in scan[0]} == {issue.id, patch.id}
    assert len(next(e for e in scan[0] if e["id"] == issue.id)["payload"]) > 1
    assert all(e["evaluation"] == "not-evaluated" for e in scan[0])
    assert git(stored, "for-each-ref", "--format=%(refname) %(objectname)") == before
    scan, signed = polls(data)
    assert scan[0] == signed[0] == []
    assert scan[1]["historyEntries"] == signed[1]["historyEntries"] == 0
    # Namespace discovery is still one expensive public remotes() operation.
    assert signed[1]["calls"] == {"repo.remotes": 1, "repo.rev_parse": 1}
    a = issue.edit("second")
    b = issue.edit("third")
    c = issue.comment("while observer was down")
    scan, signed = polls(data)
    assert scan[0] == signed[0]
    assert [event["id"] for event in scan[0]] == [a, b, c]
    assert scan[1]["historyEntries"] == 4  # full history returned, not three new ops only
    assert polls(data)[0][0] == []


def test_state_observer_collapses_edits_but_feed_keeps_ignored_review(data):
    p, peer, repo, _, base, root = data
    issue = rad.Issues.open_as(repo, p.signer()).create("before", "root")
    patches = rad.Patches.open_as(repo, p.signer())
    patch = patches.create("p", "root", base=base, head=base)
    polls(data)
    state_file = root / "state.json"
    observer.poll_once(p, repo.rid, state_file, lambda event: None)
    a, b = issue.edit("middle"), issue.edit("after")
    review = rad.Patches.open_as(repo, peer.signer()).get_mut(patch.id)
    revision = patch.patch.latest_revision
    first = review.review(revision, "accept", "first wins")
    second = review.review(revision, "reject", "recorded but not replacing")
    state_events = []
    observer.poll_once(p, repo.rid, state_file, state_events.append)
    assert state_events == []  # these are not issue/comment appearance changes
    scan, signed = polls(data)
    assert scan[0] == signed[0]
    assert {e["id"] for e in scan[0]} == {a, b, first, second}
    assert next(e for e in scan[0] if e["id"] == second)["kind"] == "patch.reviewRecorded"
    snapshot = patches.get(patch.id)
    assert snapshot.revisions[0].reviews[0].verdict == "accept"
    assert snapshot.revisions[0].reviews[0].summary == "first wins"


def test_unsigned_change_is_seen_only_by_scan_until_signing(data):
    p, _, repo, stored, _, root = data
    issue = rad.Issues.open_as(repo, p.signer()).create("t", "root")
    polls(data)
    sigref = f"refs/namespaces/{p.node_id}/refs/rad/sigrefs"
    old = git(stored, "rev-parse", sigref)
    comment = issue.comment("unsigned window")
    # Fixture-only ref manipulation; the operation remains at the COB tip, but
    # the advertised signed set is restored to its earlier state.
    git(stored, "update-ref", sigref, old)
    scan, signed = polls(data)
    assert [e["id"] for e in scan[0]] == [comment]
    assert signed[0] == []
    repo.sign_refs(p.signer())
    scan, signed = polls(data)
    assert scan[0] == []
    assert [e["id"] for e in signed[0]] == [comment]


def test_rewinds_fail_closed_then_restore_does_not_replay(data):
    p, _, repo, stored, _, root = data
    issue = rad.Issues.open_as(repo, p.signer()).create("t", "root")
    comment = issue.comment("seen")
    polls(data)
    state_file = root / "state.json"
    observer.poll_once(p, repo.rid, state_file, lambda event: None)
    ref = f"refs/namespaces/{p.node_id}/refs/cobs/xyz.radicle.issue/{issue.id}"
    git(stored, "update-ref", ref, issue.id)
    repo.sign_refs(p.signer())
    state_events = []
    observer.poll_once(p, repo.rid, state_file, state_events.append)
    assert state_events == []
    for source in ("scan", "sigrefs"):
        cursor = root / f"{source}.json"
        saved = cursor.read_bytes()
        with pytest.raises(feed.ResyncRequired):
            feed.poll_operations(p, repo.rid, cursor, lambda event: pytest.fail("partial delivery"),
                                 source=source)
        assert cursor.read_bytes() == saved
    git(stored, "update-ref", ref, comment)
    repo.sign_refs(p.signer())
    observer.poll_once(p, repo.rid, state_file, state_events.append)
    assert state_events == []
    assert polls(data)[0][0] == []
    assert polls(data)[1][0] == []
    git(stored, "update-ref", "-d", ref)
    repo.sign_refs(p.signer())
    for source in ("scan", "sigrefs"):
        cursor = root / f"{source}.json"
        saved = cursor.read_bytes()
        with pytest.raises(feed.ResyncRequired, match="removed"):
            feed.poll_operations(p, repo.rid, cursor, lambda event: None, source=source)
        assert cursor.read_bytes() == saved
    git(stored, "update-ref", ref, comment)
    repo.sign_refs(p.signer())
    assert polls(data)[0][0] == []


def test_operation_batch_callback_failure_and_source_mismatch(data):
    p, _, repo, _, _, root = data
    rad.Issues.open_as(repo, p.signer()).create("t", "root")
    path = root / "cursor.json"
    seen = []

    def fail(event):
        seen.append(event)
        raise RuntimeError("delivery")

    with pytest.raises(RuntimeError):
        feed.poll_operations(p, repo.rid, path, fail, replay=True)
    assert not path.exists()
    again = []
    feed.poll_operations(p, repo.rid, path, again.append, replay=True)
    assert again == seen
    original = path.read_bytes()
    with pytest.raises(ValueError):
        feed.poll_operations(p, repo.rid, path, again.append, source="sigrefs")
    assert path.read_bytes() == original
    broken = json.loads(original)
    broken["frontier"] = {"bad": []}
    path.write_text(json.dumps(broken))
    with pytest.raises(ValueError):
        feed.poll_operations(p, repo.rid, path, again.append)
