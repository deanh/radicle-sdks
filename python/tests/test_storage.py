"""Phase 2 tests: Storage and read-only Repository (PLAN §7 Phase 2).

Requires the extension built with `--features test-helpers` (repo fixtures).
"""

import pytest

import radiclepy


class TestStorage:
    def test_from_profile(self, profile, tmp_path):
        storage = radiclepy.Storage.from_profile(profile)
        assert "Storage" in repr(storage)
        # Storage lives under the profile's home.
        assert storage.path.startswith(str(tmp_path / "rad-alice"))

    def test_open_lowlevel(self, profile, tmp_path):
        # Storage.open(path, alias, node_id) — low-level ctor.
        storage = radiclepy.Storage.open(
            str(tmp_path / "home" / "storage"), "mitma", str(profile.node_id)
        )
        assert storage.repositories() == []

    def test_open_invalid_inputs(self, tmp_path, profile):
        with pytest.raises(ValueError):  # invalid alias (illegal characters)
            radiclepy.Storage.open(str(tmp_path / "s"), "not an alias!", str(profile.node_id))
        with pytest.raises(ValueError):  # invalid node id
            radiclepy.Storage.open(str(tmp_path / "s2"), "mitma", "not-a-key")

    def test_repository_missing(self, profile, tmp_path):
        from radiclepy import _test

        # A *valid* RepoId that isn't in this storage: create it in another
        # profile's storage and reference it here.
        other = radiclepy.Profile._from_seed(
            str(tmp_path / "rad-bob2"), "bob", bytes(31 - i for i in range(32))
        )
        missing = _test.init_project(other, str(tmp_path / "wd2"), "elsewhere")

        storage = radiclepy.Storage.from_profile(profile)
        assert not storage.contains(missing)
        with pytest.raises(radiclepy.RepositoryError):
            storage.repository(missing)
        with pytest.raises(ValueError):  # malformed id
            storage.repository("rad:not-an-id")

    def test_contains_and_list(self, profile, repo):
        storage = radiclepy.Storage.from_profile(profile)
        assert storage.contains(repo.rid)
        assert repo.rid in storage.repositories()


class TestRepositoryReads:
    def test_rid_and_repr(self, repo):
        assert repo.rid.startswith("rad:")
        assert "Repository" in repr(repo)

    def test_head(self, repo):
        head = repo.head
        assert head.name == "refs/heads/master"
        assert len(head.oid) == 40
        int(head.oid, 16)

    def test_identity_doc(self, repo):
        doc = repo.identity()
        assert doc.name == "fixture"
        assert doc.description == "a fixture repo"
        assert doc.default_branch == "master"
        assert doc.version == 1
        assert doc.visibility == "public"
        assert len(doc.delegates) == 1
        assert doc.delegates[0].startswith("did:key:")
        assert doc.threshold == 1
        assert len(doc.commit) == 40 and len(doc.blob) == 40

    def test_description(self, repo):
        assert repo.description() == "a fixture repo"

    def test_branches(self, repo):
        branches = repo.branches()
        assert branches == ["master"]
        assert repo.branch_head("master") == repo.head.oid
        assert repo.branch_head("nope") is None

    def test_branch_head_invalid_name(self, repo):
        # Binding-invalid names → ValueError (our validation), NOT None and
        # NOT GitError (review §2.3: only NotFound maps to None).
        with pytest.raises(ValueError):
            repo.branch_head("bad name")
        with pytest.raises(ValueError):
            repo.branch_head("")

    def test_rev_parse(self, repo):
        head = repo.head
        assert repo.rev_parse("master") == head.oid
        assert repo.rev_parse(head.oid) == head.oid
        # Invalid/unresolved revisions → GitError (never ValueError — §5.8)
        with pytest.raises(radiclepy.GitError):
            repo.rev_parse("nope")

    def test_commit(self, repo):
        commit = repo.commit(repo.head.oid)
        assert commit.oid == repo.head.oid
        assert commit.message == b"fixture: initial commit\n"
        assert commit.message_str.endswith("initial commit\n")
        assert commit.author_name == b"radiclepy-fixture"
        assert commit.author_email == b"fixture@example.com"
        assert commit.author_time > 0
        assert isinstance(commit.author_offset_min, int)
        assert len(commit.tree) == 40
        assert commit.parents == []  # root commit
        assert commit.encoding is None
        with pytest.raises(radiclepy.GitError):
            repo.commit("0" * 40)  # missing

    def test_tree_canonical_order(self, repo):
        commit = repo.commit(repo.head.oid)
        tree = repo.tree(commit.tree)
        names = [e.name for e in tree.entries]
        # Canonical git order: 'README.md' (0x52) < 'assets' (0x61) < 'zoo.txt'.
        assert names == [b"README.md", b"assets", b"zoo.txt"]
        assert len(tree) == 3
        kinds = {e.name: (e.kind, e.mode) for e in tree.entries}
        assert kinds[b"assets"] == ("tree", 0o040000)
        assert kinds[b"zoo.txt"] == ("blob", 0o100644)
        entry_by_name = {e.name: e for e in tree.entries}
        logo_parent = repo.tree(entry_by_name[b"assets"].oid)
        assert [e.name for e in logo_parent.entries] == [b"logo.svg"]

    def test_blob(self, repo):
        commit = repo.commit(repo.head.oid)
        tree = repo.tree(commit.tree)
        readme = next(e for e in tree.entries if e.name == b"README.md")
        content = repo.blob(readme.oid)
        assert content == b"# fixture\n\nhermetic test repository\n"

    def test_history_single_commit(self, repo):
        page = repo.history("master", limit=10)
        assert page.head == repo.head.oid
        assert page.oids == [repo.head.oid]
        assert page.offset == 0
        assert page.has_more is False

    def test_history_offset_beyond_end(self, repo):
        page = repo.history("master", limit=10, offset=100)
        assert page.oids == []
        assert page.has_more is False

    def test_history_page_limited(self, repo):
        page = repo.history("master", limit=1)
        assert len(page.oids) == 1
        assert page.has_more is False  # exactly one commit exists

    def test_history_invalid_rev(self, repo):
        with pytest.raises(radiclepy.GitError):
            repo.history("nope", limit=5)

    def test_history_limit_validation(self, repo):
        # limit == 0 raises instead of silently becoming 1 (review).
        with pytest.raises(ValueError):
            repo.history("master", limit=0)
        # Oversized limits are clamped, not rejected.
        page = repo.history("master", limit=10**6)
        assert len(page.oids) == 1

    def test_history_negative_pagination_raises_value_error(self, repo):
        # Signed parameters + explicit validation: negatives are ValueError,
        # never an OverflowError from usize conversion (review round 2).
        with pytest.raises(ValueError):
            repo.history("master", limit=-1)
        with pytest.raises(ValueError):
            repo.history("master", offset=-1)

    def test_remotes(self, repo, profile):
        from radiclepy import _test

        remotes = repo.remotes()
        assert len(remotes) == 1
        assert remotes[0].fetcher == _test.local_remote_id(profile)
        heads = remotes[0].heads
        assert any(h.startswith("refs/heads/") for h in heads)
        assert "refs/rad/id" in heads

    def test_tags_empty(self, repo):
        assert repo.tags() == []


class TestTags:
    def _tags(self, profile, repo):
        from radiclepy import _test

        _test.create_tag(profile, repo.rid, "v1", "master", annotated=True)
        _test.create_tag(profile, repo.rid, "plain", "master", annotated=False)
        return {t.name: t for t in repo.tags()}

    def test_annotated_and_lightweight(self, profile, repo):
        tags = self._tags(profile, repo)
        ann, light = tags["v1"], tags["plain"]
        assert ann.kind == "annotated"
        assert light.kind == "lightweight"

    def test_peel_semantics(self, profile, repo):
        tags = self._tags(profile, repo)
        head = repo.head.oid
        ann, light = tags["v1"], tags["plain"]
        # Annotated: target is a tag object, peeled is the commit.
        assert ann.target != head
        assert ann.peeled == head
        assert ann.peeled_kind == "commit"
        # Lightweight: target IS the commit; peeled == target (no silent
        # commit→tree peel — the FINDINGS-02 S6 trap).
        assert light.target == head
        assert light.peeled == head
        assert light.peeled_kind == "commit"

    def test_tag_order_sorted(self, profile, repo):
        tags = self._tags(profile, repo)
        assert [t.name for t in repo.tags()] == sorted(tags.keys())
