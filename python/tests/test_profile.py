"""Profile / Home / Aliases / Signer behaviour (hermetic fixtures only)."""

from __future__ import annotations

import pytest

import radiclepy as rad


# ─── Profile init (private, seeded) ──────────────────────────────────────────

def test_profile_identity_shape(profile):
    assert profile.did.startswith("did:key:z")
    assert profile.node_id.startswith("z")
    assert profile.did == f"did:key:{profile.node_id}"
    assert repr(profile) == f"Profile({profile.did})"


def test_from_seed_is_deterministic(tmp_path, profile):
    # `profile` fixture guards the test-helpers build feature.
    seed = bytes(range(32))
    p1 = rad.Profile._from_seed(str(tmp_path / "h1"), "alice", seed)
    p2 = rad.Profile._from_seed(str(tmp_path / "h2"), "alice", seed)
    assert p1.did == p2.did == profile.did


def test_seed_length_validated(tmp_path, profile):
    with pytest.raises(ValueError):
        rad.Profile._from_seed(str(tmp_path / "h"), "alice", b"short")


def test_public_init_uses_random_seed(tmp_path):
    # Two public inits must yield different identities (OS RNG seed).
    p1 = rad.Profile.init(str(tmp_path / "r1"), "randy")
    p2 = rad.Profile.init(str(tmp_path / "r2"), "randy")
    assert p1.did != p2.did


def test_init_creates_home_layout(profile):
    import pathlib

    home = pathlib.Path(profile.home.path)
    assert (home / "keys").exists()
    assert (home / "storage").exists()


def test_bad_alias_rejected_no_side_effects(tmp_path):
    target = tmp_path / "h"
    with pytest.raises(ValueError):
        rad.Profile.init(str(target), "bad alias with spaces")
    # Validation must precede filesystem state (review §7).
    assert not target.exists()


# ─── Home ────────────────────────────────────────────────────────────────────

def test_home_new_and_load(tmp_path):
    home = rad.Home(str(tmp_path / "rad"))
    assert home.path == str((tmp_path / "rad").resolve())
    again = rad.Home.load(str(tmp_path / "rad"))
    assert again.path == home.path
    assert repr(home) == f"Home({home.path})"


def test_home_resolve(monkeypatch, tmp_path):
    env_rad = tmp_path / "env-rad"
    env_rad.mkdir()
    monkeypatch.setenv("RAD_HOME", str(env_rad))
    assert rad.Home.resolve().path == str(env_rad.resolve())


# ─── Signer ──────────────────────────────────────────────────────────────────

def test_signer_matches_profile(profile):
    signer = profile.signer()
    assert signer.did == profile.did
    assert signer.node_id == profile.node_id
    assert repr(signer) == f"Signer({profile.node_id})"


# ─── Aliases ─────────────────────────────────────────────────────────────────

def test_aliases_lookup_facade(profile):
    aliases = profile.aliases
    # Upstream seeds the profile's own alias for its node id (verified
    # behaviour — asserted strictly; the facade still cannot enumerate).
    assert aliases.get(profile.node_id) == "alice"
    assert aliases.reverse_lookup("alice").get("alice") == [profile.node_id]
    assert aliases.reverse_lookup("nobody") == {}


def test_aliases_validate_inputs(profile):
    with pytest.raises(ValueError):
        profile.aliases.get("not-a-node-id")
    with pytest.raises(ValueError):
        profile.aliases.reverse_lookup("bad alias!!")


# ─── profile.home ────────────────────────────────────────────────────────────

def test_profile_home_matches_init_path(tmp_path):
    p = rad.Profile.init(str(tmp_path / "my-rad"), "homer")
    assert p.home.path == str((tmp_path / "my-rad").resolve())
