"""Phase 5 — narrow upstream-shaped repository writes (PLAN §3).

Every write maps 1:1 to an upstream `WriteRepository`/`SignRepository`
method. The three non-signing writes take NO signer (upstream doesn't);
`sign_refs` is the only signer-taking write. Fixtures are created via the
feature-gated `_test` helpers (same store internals these public writes use).
"""

import re

import pytest

import radiclepy as rad

HEX40 = re.compile(r"^[0-9a-f]{40}$")


class TestSetDefaultBranchToCanonicalHead:
    def test_canonical_head_noop(self, repo):
        # The fixture repo's default branch is already canonical: old == new,
        # not "updated" (upstream is_updated semantics).
        result = repo.set_default_branch_to_canonical_head()
        assert isinstance(result, rad.SetHead)
        assert result.new == repo.head.oid
        assert result.old == repo.head.oid
        assert result.is_updated() is False
        assert "SetHead(" in repr(result)

    def test_default_branch_ref_still_resolves(self, repo):
        repo.set_default_branch_to_canonical_head()
        # Default branch head is unchanged and still resolvable via rev_parse.
        assert repo.rev_parse("master") == repo.head.oid


class TestSetIdentityHead:
    def test_sets_and_is_idempotent(self, repo):
        first = repo.set_identity_head()
        assert HEX40.match(first)
        # Re-running recomputes the same canonical head (idempotent).
        assert repo.set_identity_head() == first


class TestSetRemoteIdentityRoot:
    def test_returns_root_oid(self, repo, profile):
        root = repo.set_remote_identity_root(profile.node_id)
        assert HEX40.match(root)

    def test_unknown_remote_writes_idempotently(self, repo, peer_profile):
        # Upstream writes the remote's identity-root ref unconditionally —
        # an unknown remote simply gets the ref created (no membership
        # check); repeat calls are idempotent.
        first = repo.set_remote_identity_root(peer_profile.node_id)
        assert HEX40.match(first)
        assert repo.set_remote_identity_root(peer_profile.node_id) == first

    def test_invalid_node_id_is_value_error(self, repo):
        # Binding-level validator → ValueError (PLAN error convention).
        with pytest.raises(ValueError, match="invalid NodeId"):
            repo.set_remote_identity_root("not-a-node-id")


class TestSignRefs:
    def test_signs_canonical_refs(self, repo, profile):
        signed = repo.sign_refs(profile.signer())
        assert isinstance(signed, rad.SignedRefs)
        assert signed.node_id == profile.node_id
        assert HEX40.match(signed.at)
        assert signed.parent is None  # first sigrefs commit is a root
        assert signed.feature_level in {"none", "root", "parent"}  # 0.25.1 has exactly these
        refs = signed.refs()
        assert isinstance(refs, dict) and refs
        assert all(HEX40.match(oid) for oid in refs.values())
        # The signed set covers the canonical refs; master is one of them.
        values = set(refs.values())
        assert repo.head.oid in values

    def test_unchanged_repo_reuses_signed_commit(self, repo, profile):
        first = repo.sign_refs(profile.signer())
        second = repo.sign_refs(profile.signer())
        # Upstream does not push a new sigref commit when nothing changed.
        assert second.at == first.at
        assert "SignedRefs(" in repr(second)

    def test_requires_signer_type(self, repo):
        with pytest.raises(TypeError):
            repo.sign_refs("z6Mkf")  # not a Signer
