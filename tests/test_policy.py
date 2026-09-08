"""Phase 3 tests: PolicyStore (PLAN §7 Phase 3)."""

import pytest

import radiclepy as rad

#: Syntactically valid NodeId that no fixture actually follows.
STRAY_NID = "z6MkhaXgBZDvotDkL5257faiztiGiC2QtKLGpbnnEGta2doK"


@pytest.fixture
def policy(profile):
    return profile.policy_store()


class TestConstruction:
    def test_from_profile_and_path(self, profile, policy):
        assert policy.path.endswith("policies.db")
        assert "rw" in repr(policy)

    def test_reader(self, profile):
        reader = rad.PolicyStore.reader(profile)
        assert "ro" in repr(reader)

    def test_reader_sees_committed_writes(self, policy, profile):
        from radiclepy import _test

        reader = rad.PolicyStore.reader(profile)
        nid = _test.local_remote_id(profile)
        assert policy.follow(nid) is True
        assert reader.is_following(nid) is True

    def test_reader_rejects_writes(self, profile):
        from radiclepy import _test

        reader = rad.PolicyStore.reader(profile)
        with pytest.raises(rad.StorageError):
            reader.follow(_test.local_remote_id(profile))

    def test_open_lowlevel(self, tmp_path):
        store = rad.PolicyStore.open(str(tmp_path / "policies.db"))
        assert store.follow_policies() == []

    def test_invalid_inputs_raise_value_error(self, policy):
        with pytest.raises(ValueError):
            policy.follow("not-a-node-id")
        with pytest.raises(ValueError):
            policy.seed("rad:garbage")
        with pytest.raises(ValueError):
            policy.set_follow_policy(STRAY_NID, "permit")  # bad policy word
        with pytest.raises(ValueError):
            policy.seed_policy("rad:z" + "q" * 50)  # malformed RepoId
        with pytest.raises(ValueError):
            policy.set_seed_policy("rad:" + "z" + "q" * 50, "maybe")


class TestFollow:
    def test_follow_unfollow(self, policy, profile):
        from radiclepy import _test

        nid = _test.local_remote_id(profile)
        assert policy.follow(nid) is True       # store changed
        assert policy.follow(nid) is False      # already followed
        assert policy.is_following(nid) is True
        assert policy.unfollow(nid) is True
        assert policy.unfollow(nid) is False
        assert policy.is_following(nid) is False

    def test_follow_with_alias(self, policy, profile):
        from radiclepy import _test

        nid = _test.local_remote_id(profile)
        assert policy.follow(nid, alias="alice") is True
        fp = policy.follow_policy(nid)
        assert fp is not None
        assert fp.node_id == nid
        assert fp.alias == "alice"
        assert fp.policy == "allow"
        assert "alice" in repr(fp)

    def test_follow_invalid_alias(self, policy, profile):
        from radiclepy import _test

        with pytest.raises(ValueError):
            policy.follow(_test.local_remote_id(profile), alias="not an alias!")

    def test_invalid_scope(self, policy):
        with pytest.raises(ValueError):
            policy.seed("rad:" + "z" + "q" * 50, scope="sometimes")

    def test_unfollow_removes_block_rows_too(self, policy, profile):
        # Upstream unfollow is an unconditional DELETE: it removes block
        # rows as well as allow rows (review correction).
        from radiclepy import _test

        nid = _test.local_remote_id(profile)
        policy.set_follow_policy(nid, "block")
        assert policy.unfollow(nid) is True
        assert policy.is_blocked(nid) is False
        assert policy.follow_policy(nid) is None

    def test_unblock_nid_only_touches_block_rows(self, policy, profile):
        from radiclepy import _test

        nid = _test.local_remote_id(profile)
        policy.follow(nid)  # allow row
        assert policy.unblock_nid(nid) is False   # not a block row
        assert policy.is_following(nid) is True   # untouched
        policy.set_follow_policy(nid, "block")
        assert policy.unblock_nid(nid) is True

    def test_block_and_unblock(self, policy, profile):
        from radiclepy import _test

        nid = _test.local_remote_id(profile)
        assert policy.set_follow_policy(nid, "block") is True
        assert policy.is_blocked(nid) is True
        assert policy.is_following(nid) is False  # blocked != following
        assert policy.follow_policy(nid).policy == "block"
        assert policy.unblock_nid(nid) is True
        assert policy.is_blocked(nid) is False

    def test_follow_policies_list(self, policy, profile):
        from radiclepy import _test

        nid = _test.local_remote_id(profile)
        policy.follow(nid, alias="alice")
        entries = policy.follow_policies()
        assert [e.node_id for e in entries] == [nid]
        assert entries[0].alias == "alice"

    def test_nodes_by_alias_fuzzy(self, policy, profile):
        from radiclepy import _test

        nid = _test.local_remote_id(profile)
        policy.follow(nid, alias="alice")
        hits = policy.nodes_by_alias("lic")  # substring match (upstream LIKE)
        assert hits == [(nid, "alice")]
        assert policy.nodes_by_alias("nobody") == []


class TestSeed:
    @pytest.fixture
    def rid(self, profile, tmp_path):
        from radiclepy import _test

        return _test.init_project(profile, str(tmp_path / "wd"), "fixture")

    def test_seed_roundtrip(self, policy, rid):
        assert policy.seed(rid) is True
        assert policy.seed(rid) is False
        assert policy.is_seeding(rid) is True
        sp = policy.seed_policy(rid)
        assert sp.policy == "allow"
        assert sp.scope == "all"  # default scope
        assert "scope=all" in repr(sp)
        assert policy.unseed(rid) is True
        assert policy.is_seeding(rid) is False

    def test_seed_scope_followed(self, policy, rid):
        policy.seed(rid, scope="followed")
        assert policy.seed_policy(rid).scope == "followed"

    def test_block_seed_has_no_scope(self, policy, rid):
        assert policy.set_seed_policy(rid, "block") is True
        sp = policy.seed_policy(rid)
        assert sp.policy == "block"
        assert sp.scope is None
        assert policy.is_seeding(rid) is False
        assert policy.unblock_rid(rid) is True

    def test_unseed_removes_block_rows_too(self, policy, rid):
        # Upstream unseed is an unconditional DELETE (review correction).
        policy.set_seed_policy(rid, "block")
        assert policy.unseed(rid) is True
        assert policy.seed_policy(rid) is None

    def test_unblock_rid_only_touches_block_rows(self, policy, rid):
        policy.seed(rid)  # allow row
        assert policy.unblock_rid(rid) is False
        assert policy.is_seeding(rid) is True
        policy.set_seed_policy(rid, "block")
        assert policy.unblock_rid(rid) is True

    def test_seed_policies_list(self, policy, rid):
        policy.seed(rid, scope="followed")
        entries = policy.seed_policies()
        assert [e.rid for e in entries] == [rid]
        assert entries[0].scope == "followed"
