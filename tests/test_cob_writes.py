"""Phase 5 — typed COB writes (Issues/Patches).

Signer policy (PLAN §3): the signer is fixed at store-open time
(`open_as(repository, signer)` — upstream `WriteAs(signer)`); mutating
methods take NO signer. Every mutator commits one COB transaction.
"""

import re

import pytest

import radiclepy as rad

HEX40 = re.compile(r"^[0-9a-f]{40}$")


@pytest.fixture
def issues_w(repo, profile):
    return rad.Issues.open_as(repo, profile.signer())


@pytest.fixture
def patches_w(repo, profile):
    return rad.Patches.open_as(repo, profile.signer())


class TestStoreModes:
    def test_open_is_read_only(self, repo):
        issues = rad.Issues.open(repo)
        assert "read-only" in repr(issues)
        with pytest.raises(ValueError, match="read-only"):
            issues.create("t", "d")
        patches = rad.Patches.open(repo)
        with pytest.raises(ValueError, match="read-only"):
            patches.create("t", "d", "delegates", repo.head.oid, repo.head.oid)

    def test_open_as_is_write(self, repo, profile):
        issues = rad.Issues.open_as(repo, profile.signer())
        assert ", write)" in repr(issues)


class TestIssueWrites:
    def test_create_and_snapshot(self, issues_w, repo):
        m = issues_w.create("hello", "desc", labels=["bug"])
        assert HEX40.match(m.id)
        assert m.issue.title == "hello"
        assert m.issue.description == "desc"
        assert m.issue.state == "open"
        assert m.issue.labels == ["bug"]
        # Visible through the read-only store too (immediately persisted).
        got = rad.Issues.open(repo).get(m.id)
        assert got is not None and got.title == "hello"

    def test_mutators_commit_one_op_each(self, issues_w, repo):
        m = issues_w.create("t0", "d")
        e1 = m.comment("c1")
        e2 = m.edit("t1")
        e3 = m.lifecycle("closed", "solved")
        for entry in (e1, e2, e3):
            assert HEX40.match(entry)
        assert m.issue.title == "t1"
        assert m.issue.state == "closed"
        assert m.issue.close_reason == "solved"
        # Raw causal history: create + 3 ops (each op persisted separately).
        ops = rad.Raw.open(repo).history("xyz.radicle.issue", m.id)
        assert len(ops) == 4
        # Topological: every op's parents precede it.
        seen = []
        for op in ops:
            assert all(p in seen for p in op.parents)
            seen.append(op.id)

    def test_comment_reply_to_defaults_to_root(self, issues_w):
        m = issues_w.create("t", "d")
        root = m.issue.comment_ids[0]
        m.comment("reply-a")
        m.comment("reply-b", reply_to=root)
        assert m.issue.comment_count == 3
        # threaded reply chain is visible in comment ids
        assert len(set(m.issue.comment_ids)) == 3

    def test_label_replaces(self, issues_w):
        # Upstream Action::Label SETS the label set (replace, not add).
        m = issues_w.create("t", "d", labels=["bug"])
        m.label(["urgent"])
        assert m.issue.labels == ["urgent"]
        m.label(["bug", "urgent"])
        assert m.issue.labels == ["bug", "urgent"]

    def test_edit_description_and_assign(self, issues_w, profile):
        m = issues_w.create("t", "old")
        m.edit_description("new")
        assert m.issue.description == "new"
        did = profile.signer().did
        m.assign([did])
        assert m.issue.assignees == [did]

    def test_lifecycle_validation(self, issues_w):
        m = issues_w.create("t", "d")
        with pytest.raises(ValueError, match="invalid state"):
            m.lifecycle("bogus")
        with pytest.raises(ValueError, match="invalid close reason"):
            m.lifecycle("closed", "bogus")
        with pytest.raises(ValueError, match="only valid for state='closed'"):
            m.lifecycle("open", "solved")
        assert m.lifecycle("closed")  # reason defaults to "unspecified"
        assert m.issue.close_reason == "unspecified"
        assert m.lifecycle("open")

    def test_get_mut_second_handle_sees_updates(self, issues_w):
        m = issues_w.create("t", "d")
        m.comment("one")
        m2 = issues_w.get_mut(m.id)
        assert m2.issue.comment_count == 2
        m2.comment("two")
        assert issues_w.get_mut(m.id).issue.comment_count == 3

    def test_get_mut_missing_id_is_cob_error(self, issues_w):
        with pytest.raises(rad.CobError):
            issues_w.get_mut("0" * 40)

    def test_invalid_inputs_are_value_errors(self, issues_w, profile):
        m = issues_w.create("t", "d")
        with pytest.raises(ValueError):
            m.label(["NO UPPERCASE OR SPACES!"])
        with pytest.raises(ValueError):
            m.assign(["not-a-did"])
        with pytest.raises(ValueError):
            m.comment("x", reply_to="not-an-oid")
        with pytest.raises(ValueError):
            issues_w.create("", "d")  # upstream: title must not be empty
        with pytest.raises(ValueError):
            issues_w.create("multi\nline", "d")  # no newlines in titles


class TestPatchWrites:
    def test_create_open_patch(self, patches_w, repo):
        pm = patches_w.create(
            "patch A", "d", "delegates", repo.head.oid, repo.head.oid, labels=["x"]
        )
        assert HEX40.match(pm.id)
        assert pm.patch.state == "open"
        assert pm.patch.target == "delegates"
        assert pm.patch.labels == ["x"]
        assert pm.patch.base == repo.head.oid == pm.patch.head
        assert pm.patch.revision_count == 1

    def test_draft_with_branch_target_roundtrips(self, patches_w, repo):
        pm = patches_w.draft(
            "patch B", "d", "branch:release", repo.head.oid, repo.head.oid
        )
        assert pm.patch.state == "draft"
        # input "branch:<name>" ⇢ stored refs/heads/<name> ⇢ output "branch:<name>"
        assert pm.patch.target == "branch:release"

    def test_created_patches_are_readable(self, patches_w, repo):
        pm = patches_w.create("p", "d", "delegates", repo.head.oid, repo.head.oid)
        got = rad.Patches.open(repo).get(pm.id)
        assert got is not None and got.title == "p" and got.state == "open"
        assert pm.id in [x.id for x in rad.Patches.open(repo).all()]

    def test_invalid_target_and_oids(self, patches_w, repo):
        with pytest.raises(ValueError, match="delegates.*branch"):
            patches_w.draft("t", "d", "bogus", repo.head.oid, repo.head.oid)
        with pytest.raises(ValueError, match="invalid git OID"):
            patches_w.draft("t", "d", "delegates", "zz", repo.head.oid)
