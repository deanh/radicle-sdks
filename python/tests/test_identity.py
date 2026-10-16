"""Identifier validators: canonicalization passes, junk raises ValueError."""

from __future__ import annotations

import pytest

import radiclepy as rad

# Real rid from the sibling project's docs (checksummed base58btc).
VALID_RID = "rad:z4VYyJ9KuwMNkXGQnmKuGPGKw3inv"


def test_parse_repo_id_canonical():
    assert rad.parse_repo_id(VALID_RID) == VALID_RID


def test_parse_did_roundtrip(profile):
    assert rad.parse_did(profile.did) == profile.did


def test_parse_node_id_roundtrip(profile):
    assert rad.parse_node_id(profile.node_id) == profile.node_id


def test_parse_oid(profile):
    oid40 = "0" * 40
    assert rad.parse_oid(oid40) == oid40
    with pytest.raises(ValueError):
        rad.parse_oid("not-hex")
    with pytest.raises(ValueError):
        rad.parse_oid("0" * 39)


def test_parse_object_id(profile):
    obj40 = "a" * 40
    assert rad.parse_object_id(obj40) == obj40
    with pytest.raises(ValueError):
        rad.parse_object_id("nope")


@pytest.mark.parametrize(
    "fn,bad",
    [
        (rad.parse_repo_id, "rad:not base58 !!"),
        (rad.parse_did, "did:key:not-a-key!!"),
        (rad.parse_node_id, "not-a-node-id"),
    ],
)
def test_invalid_raise_value_error(fn, bad):
    with pytest.raises(ValueError):
        fn(bad)


def test_underscored_junk_stays_value_error(profile):
    # ValueError, NOT radiclepy.Error subclasses — validators are syntactic
    # (PLAN §5.8).
    with pytest.raises(ValueError) as excinfo:
        rad.parse_oid("zzz")
    assert not issubclass(excinfo.type, rad.Error)
