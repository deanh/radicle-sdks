"""Test fixtures (PLAN §5.7 — committed fixture strategy).

Strategy:
  * Hermetic: every fixture uses an explicit temp path. NO process-global
    RAD_HOME mutation anywhere (parallel-pytest safe).
  * Deterministic: identities come from `Profile._from_seed` (private,
    test-only) with fixed seeds — no ssh-agent, no RAD_PASSPHRASE, no RNG.
  * Unencrypted keystores only (passphrase=None).
"""

from __future__ import annotations

import pytest

import radiclepy as rad

#: Fixed 32-byte seeds for fixture identities. Test-only; never production.
SEEDS: dict[str, bytes] = {
    "alice": bytes(range(32)),
    "bob": bytes(31 - i for i in range(32)),
}


def _profile(tmp_path, alias: str) -> rad.Profile:
    if not hasattr(rad.Profile, "_from_seed"):
        pytest.skip(
            "deterministic fixtures require the test build: "
            "maturin develop --features test-helpers"
        )
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
