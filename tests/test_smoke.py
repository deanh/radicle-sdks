"""Smoke tests: import surface matches the package contract."""

from __future__ import annotations

import radiclepy as rad


def test_public_surface():
    for name in rad.__all__:
        assert hasattr(rad, name), f"missing public name: {name}"


def test_exception_hierarchy():
    children = [
        rad.StorageError,
        rad.RepositoryError,
        rad.GitError,
        rad.DatabaseError,
        rad.IdentityError,
        rad.SignerError,
        rad.PolicyError,
        rad.CobError,
    ]
    for exc in children:
        assert issubclass(exc, rad.Error)
    assert issubclass(rad.Error, Exception)
    # Siblings are distinct types.
    assert len(set(children)) == len(children)


def test_native_module_is_bundled():
    # Hybrid layout: the compiled module lives inside the package.
    from radiclepy import radiclepy as native  # noqa: F401
