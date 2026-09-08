"""Phase 3 tests: RoutingStore (PLAN §7 Phase 3).

RoutingStore is read-only; entries are seeded via the feature-gated
`_test.add_routing_entry` helper (mutable database writes are Phase 5).
"""

import pathlib

import pytest

import radiclepy as rad

MS = 1_700_000_000_000  # fixed fixture timestamp (epoch millis)


class TestOpen:
    def test_initialized_home_opens_empty(self, profile):
        # Profile::init creates AND migrates node.db, so a freshly
        # initialized profile opens with an empty routing table.
        store = rad.RoutingStore.from_profile(profile)
        assert store.total() == 0
        assert store.is_empty() is True

    def test_missing_db_raises(self, profile):
        # Regression (review): Database::reader opens READ-ONLY and creates
        # nothing — with node.db removed, opening is a DatabaseError.
        db = pathlib.Path(profile.home.path) / "node" / "node.db"
        assert db.exists()  # created by Profile::init
        db.unlink()
        with pytest.raises(rad.DatabaseError):
            rad.RoutingStore.from_profile(profile)

    def test_invalid_ids(self, profile, tmp_path):
        from radiclepy import _test

        rid = _test.init_project(profile, str(tmp_path / "wd"), "fixture")
        _test.add_routing_entry(profile, rid, _test.local_remote_id(profile))
        store = profile.routing_store()
        with pytest.raises(ValueError):
            store.get("rad:garbage")
        with pytest.raises(ValueError):
            store.get_inventory("not-a-node-id")


class TestReads:
    @pytest.fixture
    def seeded(self, profile, tmp_path):
        from radiclepy import _test

        rid_a = _test.init_project(profile, str(tmp_path / "a"), "repo-a")
        rid_b = _test.init_project(profile, str(tmp_path / "b"), "repo-b")
        rid_c = _test.init_project(profile, str(tmp_path / "c"), "repo-c")
        nid = _test.local_remote_id(profile)
        assert _test.add_routing_entry(profile, rid_a, nid, MS) is True
        assert _test.add_routing_entry(profile, rid_b, nid, MS + 1000) is True
        return rid_a, rid_b, rid_c, nid

    def test_reads(self, profile, seeded):
        rid_a, rid_b, rid_c, nid = seeded
        store = rad.RoutingStore.from_profile(profile)
        assert store.get(rid_a) == [nid]
        assert store.get(rid_c) == []  # valid rid, no routing entries
        assert sorted(store.get_inventory(nid)) == sorted([rid_a, rid_b])
        assert store.count(rid_a) == 1
        assert store.total() == 2
        assert store.is_empty() is False

    def test_entry_timestamp(self, profile, seeded, tmp_path):
        rid_a, _, _, nid = seeded
        other = rad.Profile._from_seed(
            str(tmp_path / "rad-bob2"), "bob", bytes(31 - i for i in range(32))
        )
        from radiclepy import _test

        stray_nid = _test.local_remote_id(other)
        store = profile.routing_store()
        assert store.entry(rid_a, nid) == MS
        assert store.entry(rid_a, stray_nid) is None

    def test_entries(self, profile, seeded):
        rid_a, rid_b, _rid_c, nid = seeded
        store = profile.routing_store()
        entries = store.entries()
        assert [(e.rid, e.node_id, e.timestamp) for e in entries] == sorted(
            [(rid_a, nid, MS), (rid_b, nid, MS + 1000)]
        )
        assert "RoutingEntry" in repr(entries[0])

    def test_repr(self, profile, seeded):
        assert "RoutingStore" in repr(profile.routing_store())
