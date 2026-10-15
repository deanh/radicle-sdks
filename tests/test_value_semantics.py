"""Structural snapshots versus identity handles, using public release APIs."""
from pathlib import Path
import sqlite3

import pytest
import radiclepy as rad
from workflow_smoke import project_fixture, git


def test_all_snapshot_equality_and_hashes(tmp_path):
    profile, _, _, repo, _, stored, base = project_fixture(tmp_path)
    policy = profile.policy_store()
    policy.follow(profile.node_id)
    policy.seed(repo.rid)
    with sqlite3.connect(Path(profile.home.path) / "node/node.db") as db:
        db.execute("INSERT INTO routing (repo, node, timestamp) VALUES (?, ?, ?)",
                   (repo.rid, profile.node_id, 1700000000000))
    git(stored, "tag", "v1", base)
    issue = rad.Issues.open_as(repo, profile.signer()).create("title", "body")
    patch = rad.Patches.open_as(repo, profile.signer()).create(
        "title", "desc", base=base, head=base, target="delegates")
    patch.review(patch.patch.latest_revision, "accept", "summary")
    raw = rad.Raw.open(repo)

    def snapshots():
        i = rad.Issues.open(repo).get(issue.id)
        p = rad.Patches.open(repo).get(patch.id)
        op = raw.history("xyz.radicle.issue", issue.id)[0]
        return [repo.head, repo.commit(base), repo.tree(repo.commit(base).tree),
                repo.tree(repo.commit(base).tree).entries[0], repo.tags()[0],
                repo.remotes()[0], repo.identity(), repo.history("HEAD"),
                repo.set_default_branch_to_canonical_head(), repo.sign_refs(profile.signer()),
                i, p, i.comments[0], p.revisions[0], p.revisions[0].reviews[0],
                op, op.manifest, raw.refs("xyz.radicle.issue", issue.id)[0],
                policy.follow_policies()[0], policy.seed_policies()[0],
                profile.routing_store().entries()[0]]

    first, second = snapshots(), snapshots()
    assert len({type(v) for v in first}) == 21
    for left, right in zip(first, second):
        assert left is not right and left == right and not (left != right)
        assert hash(left) == hash(right)
        assert len({left, right}) == 1 and {left: "value"}[right] == "value"
        assert left != object() and left != "unrelated"
        assert left.__eq__(object()) is NotImplemented
        before = hash(left)
        for name in dir(left):
            if name.startswith("_"):
                continue
            value = getattr(left, name)
            if isinstance(value, list):
                value.clear()  # getters expose copies, including nested snapshots
        if isinstance(left, rad.SignedRefs):
            left.refs().clear()
        if isinstance(left, rad.RawOp):
            for action in left.actions_json():
                if isinstance(action, dict):
                    action.clear()
        assert left == right and hash(left) == before
        with pytest.raises(AttributeError):
            left.unknown_field = 1


def test_changed_and_nested_snapshots_are_not_equal(tmp_path):
    profile, _, _, repo, _, _, base = project_fixture(tmp_path)
    writer = rad.Issues.open_as(repo, profile.signer())
    issue = writer.create("before", "body")
    old = issue.issue
    issue.edit("after")
    new = issue.issue
    assert old.id == new.id and old != new
    assert {old: "old", new: "new"}[old] == "old"
    patch = rad.Patches.open_as(repo, profile.signer()).create(
        "title", "description", base=base, head=base, target="delegates")
    before = patch.patch
    patch.review(before.latest_revision, "accept", "new nested review")
    after = patch.patch
    assert (before.id, before.head, before.description, before.version) == (
        after.id, after.head, after.description, after.version)
    assert before != after and before.revisions[0] != after.revisions[0]
    assert before.revisions[0].reviews == []


def test_handles_keep_identity_semantics(tmp_path, monkeypatch):
    profile, _, storage, repo, _, _, base = project_fixture(tmp_path)
    monkeypatch.setenv("RAD_HOME", profile.home.path)
    issues = rad.Issues.open_as(repo, profile.signer())
    issue = issues.create("t", "d")
    patches = rad.Patches.open_as(repo, profile.signer())
    patch = patches.create("t", "d", base=base, head=base)
    pairs = [(profile, rad.Profile.load()), (profile.aliases, profile.aliases),
             (issue, issues.get_mut(issue.id)), (patch, patches.get_mut(patch.id)),
             (profile.storage, profile.storage),
             (storage.repository(repo.rid), storage.repository(repo.rid)),
             (profile.home, profile.home), (profile.signer(), profile.signer()),
             (profile.policy_store(), profile.policy_store()),
             (profile.routing_store(), profile.routing_store()),
             (rad.Issues.open(repo), rad.Issues.open(repo)),
             (rad.Patches.open(repo), rad.Patches.open(repo)),
             (rad.Raw.open(repo), rad.Raw.open(repo))]
    for left, right in pairs:
        assert left == left and left != right
        assert len({left, right}) == 2
