"""Test fixtures (PLAN §5.7 — committed fixture strategy).

Strategy:
  * Hermetic: every fixture uses an explicit temp path. NO process-global
    RAD_HOME mutation anywhere (parallel-pytest safe).
  * Deterministic: identities come from `Profile._from_seed` (private,
    test-only) with fixed seeds — no ssh-agent, no RAD_PASSPHRASE, no RNG.
  * Unencrypted keystores only (passphrase=None).
"""

from __future__ import annotations

import os

import pytest

import radiclepy as rad

#: Fixed 32-byte seeds for fixture identities. Test-only; never production.
SEEDS: dict[str, bytes] = {
    "alice": bytes(range(32)),
    "bob": bytes(31 - i for i in range(32)),
}


def _profile(tmp_path, alias: str) -> rad.Profile:
    if not hasattr(rad.Profile, "_from_seed"):
        # A plain `uv run pytest` can re-sync the project and rebuild the
        # extension without the feature, which would turn these into silent
        # skips. CI sets RADICLE_REQUIRE_TEST_HELPERS=1 to fail instead.
        message = (
            "deterministic fixtures require the test build: run "
            "`uv run maturin develop --uv --features test-helpers`, then "
            "`uv run --no-sync pytest` so uv does not rebuild without it"
        )
        if os.environ.get("RADICLE_REQUIRE_TEST_HELPERS") == "1":
            pytest.fail(message)
        pytest.skip(message)
    home = tmp_path / f"rad-{alias}"
    return rad.Profile._from_seed(str(home), alias, SEEDS[alias])


@pytest.fixture
def profile(tmp_path) -> rad.Profile:
    """A deterministic, unencrypted, hermetic profile (alias "alice")."""
    return _profile(tmp_path, "alice")


@pytest.fixture
def peer_profile(tmp_path) -> rad.Profile:
    """A second deterministic profile to exercise multi-identity cases."""
    return _profile(tmp_path, "bob")


@pytest.fixture
def repo(profile, tmp_path):
    """A storage repository with one deterministic fixture commit.

    Created via the feature-gated test helpers (Phase 2 has no public write
    surface; writes are Phase 5).
    """
    from radiclepy import _test

    workdir = tmp_path / "workdir"
    rid = _test.init_project(profile, str(workdir), "fixture", "a fixture repo")
    storage = rad.Storage.from_profile(profile)
    return storage.repository(rid)


@pytest.fixture
def now():
    """Wall-clock seconds at fixture creation (for timestamp-unit checks)."""
    import time

    return int(time.time())
