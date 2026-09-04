"""Python bindings for the radicle crate.

The implementation is the compiled extension module ``radiclepy.radiclepy``;
this package surface re-exports the public API so ``radiclepy.Profile`` etc.
work without the inner prefix.
"""

from __future__ import annotations

from .radiclepy import (  # type: ignore[attr-defined]
    Aliases,
    CobError,
    DatabaseError,
    Error,
    GitError,
    Home,
    IdentityError,
    PolicyError,
    Profile,
    RepositoryError,
    Signer,
    SignerError,
    StorageError,
    parse_did,
    parse_node_id,
    parse_object_id,
    parse_oid,
    parse_repo_id,
)

__all__ = [
    "Aliases",
    "CobError",
    "DatabaseError",
    "Error",
    "GitError",
    "Home",
    "IdentityError",
    "PolicyError",
    "Profile",
    "RepositoryError",
    "Signer",
    "SignerError",
    "StorageError",
    "parse_did",
    "parse_node_id",
    "parse_object_id",
    "parse_oid",
    "parse_repo_id",
]
