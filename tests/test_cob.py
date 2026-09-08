"""Phase 4 tests: typed COB readers (Issue/Patch) + raw COB inspection.

COB creation uses the feature-gated helpers (`_test.create_issue` /
`_test.create_patch`) — public COB writes are Phase 5.
"""

import json

import pytest

import radiclepy as rad

ISSUE_TN = "xyz.radicle.issue"
PATCH_TN = "xyz.radicle.patch"


@pytest.fixture
def issue(repo, profile):
    from radiclepy import _test

    return _test.create_issue(
        profile, repo.rid, "bug report", "it does a thing", labels=["bug", "urgent"]
    )


@pytest.fixture
def patch(repo, profile):
    from radiclepy import _test

    return _test.create_patch(profile, repo.rid, "my patch", "fixes stuff")


class TestIssues:
    def test_open_and_empty(self, repo):
        issues = rad.Issues.open(repo)
        assert issues.is_empty() is True
        assert issues.count() == 0
        assert issues.all() == []
        assert "Issues" in repr(issues)

    def test_get(self, repo, issue):
        iss = rad.Issues.open(repo).get(issue)
        assert iss is not None
        assert iss.id == issue
        assert iss.title == "bug report"
        assert iss.description == "it does a thing"
        assert iss.state == "open"
        assert iss.close_reason is None
        assert iss.labels == ["bug", "urgent"]  # sorted
        assert iss.author.startswith("did:key:")
        assert iss.assignees == []
        assert iss.comment_count >= 1
        assert len(set(iss.comment_ids)) == iss.comment_count
        assert "bug report" in repr(iss)

    def test_get_missing(self, repo):
        assert rad.Issues.open(repo).get("0" * 40) is None

    def test_get_invalid_id(self, repo):
        with pytest.raises(ValueError):
            rad.Issues.open(repo).get("not-an-object-id")

    def test_all_sorted(self, repo, profile):
        from radiclepy import _test

        ids = [
            _test.create_issue(profile, repo.rid, f"issue {i}", "d") for i in range(3)
        ]
        issues = rad.Issues.open(repo).all()
        assert [i.id for i in issues] == sorted(ids)

    def test_labels_validation(self, repo, profile):
        from radiclepy import _test

        with pytest.raises(ValueError):
            _test.create_issue(profile, repo.rid, "t", "d", labels=["has space"])

    def test_count_matches(self, repo, issue, patch):
        # patch is a different typename: issue count must not include it
        issues = rad.Issues.open(repo)
        assert issues.count() == 1
        assert issues.is_empty() is False


class TestPatches:
    def test_get(self, repo, patch):
        pa = rad.Patches.open(repo).get(patch)
        assert pa is not None
        assert pa.id == patch
        assert pa.title == "my patch"
        assert pa.description == "fixes stuff"
        assert pa.state == "open"  # created open, not draft
        assert pa.target == "delegates"
        assert len(pa.base) == 40 and len(pa.head) == 40
        assert pa.version == 0
        assert pa.revision_count == 1
        assert pa.labels == []
        assert "my patch" in repr(pa)

    def test_empty_and_missing(self, repo):
        patches = rad.Patches.open(repo)
        assert patches.get("0" * 40) is None
        with pytest.raises(ValueError):
            patches.get("nope")

    def test_all(self, repo, patch, issue):
        patches = rad.Patches.open(repo)
        assert [p.id for p in patches.all()] == [patch]
        assert patches.count() == 1


class TestRawCob:
    def test_typenames(self, repo, issue, patch):
        tns = rad.Raw.open(repo).typenames()
        assert ISSUE_TN in tns
        assert PATCH_TN in tns
        assert "xyz.radicle.id" in tns  # identity COB present too
        assert tns == sorted(tns)

    def test_entities_and_refs(self, repo, issue):
        raw = rad.Raw.open(repo)
        entities = raw.entities(ISSUE_TN)
        assert entities == [issue]
        refs = raw.refs(ISSUE_TN, issue)
        assert len(refs) == 1
        ref = refs[0]
        assert ref.name.endswith(f"refs/cobs/{ISSUE_TN}/{issue}")
        assert ref.name.startswith("refs/namespaces/")  # full namespaced ref
        assert len(ref.target) == 40
        assert "CobRef" in repr(ref)

    def test_entities_unknown_typename(self, repo):
        assert rad.Raw.open(repo).entities("xyz.unknown.thing") == []

    def test_invalid_typename_or_id(self, repo):
        raw = rad.Raw.open(repo)
        with pytest.raises(ValueError):
            raw.entities("Not A Valid Typename!")
        with pytest.raises(ValueError):
            raw.refs(ISSUE_TN, "not-an-id")

    def test_history_single_op(self, repo, issue):
        ops = rad.Raw.open(repo).history(ISSUE_TN, issue)
        assert len(ops) == 1
        op = ops[0]
        assert op.type_name == ISSUE_TN
        assert op.actor.startswith("z6Mk")  # canonical NodeId
        assert op.timestamp >= 0
        # Fresh issue with labels: comment + edit + label actions, one op.
        assert len(op.actions) >= 2
        assert all(isinstance(a, bytes) for a in op.actions)
        assert op.parents == []
        assert op.identity is not None  # identity COB link (NOT traversed)
        assert op.version >= 1
        for t in op.action_json_types():
            assert t == "dict"  # actions are JSON objects

    def test_history_actions_are_canonical_json(self, repo, issue):
        op = rad.Raw.open(repo).history(ISSUE_TN, issue)[0]
        for blob in op.actions:
            parsed = json.loads(blob)
            assert isinstance(parsed, dict)
            assert "type" in parsed

    def test_history_multi_op_topological(self, repo, profile):
        from radiclepy import _test

        iid = _test.create_issue(
            profile, repo.rid, "threaded", "root", comments=["a", "b", "c"]
        )
        ops = rad.Raw.open(repo).history(ISSUE_TN, iid)
        assert len(ops) == 4  # root + 3 comments
        seen = {}
        for pos, op in enumerate(ops):
            for parent in op.parents:
                assert parent in seen, "parent emitted after child"
                assert seen[parent] < pos
            seen[op.id] = pos

    def test_history_does_not_traverse_identity(self, repo, issue):
        # The causal walk must NOT pull in the identity COB's entry even
        # though the issue op links to it via `identity` (FINDINGS-0A).
        raw = rad.Raw.open(repo)
        issue_ids = {op.id for op in raw.history(ISSUE_TN, issue)}
        id_entity = raw.entities("xyz.radicle.id")[0]
        id_ids = {op.id for op in raw.history("xyz.radicle.id", id_entity)}
        assert id_ids, "identity COB should have its own entry"
        assert issue_ids.isdisjoint(id_ids)

    def test_history_unknown_entity(self, repo):
        assert rad.Raw.open(repo).history(ISSUE_TN, "0" * 40) == []

    def test_patch_history(self, repo, patch):
        ops = rad.Raw.open(repo).history(PATCH_TN, patch)
        assert len(ops) == 1
        assert ops[0].type_name == PATCH_TN
        assert len(ops[0].actions) >= 1
