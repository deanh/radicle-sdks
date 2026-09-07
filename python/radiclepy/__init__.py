"""Python bindings for the radicle crate.

The implementation is the compiled extension module ``radiclepy.radiclepy``;
this package surface re-exports the public API so ``radiclepy.Profile`` etc.
work without the inner prefix.
"""

from __future__ import annotations

from ._types import Alias, Did, NodeId, ObjectId, Oid, RepoId
from .radiclepy import (  # type: ignore[attr-defined]
    Aliases,
    CobError,
    Commit,
    DatabaseError,
    Error,
    GitError,
    HeadInfo,
    HistoryPage,
    Home,
    IdentityError,
    PolicyError,
    Profile,
    ProjectDoc,
    RemoteInfo,
    Repository,
    RepositoryError,
    Signer,
    SignerError,
    Storage,
    StorageError,
    TagInfo,
    Tree,
    TreeEntry,
    __version__,
    parse_did,
    parse_node_id,
    parse_object_id,
    parse_oid,
    parse_repo_id,
)

__all__ = [
    "Aliases",
    # identifier type aliases (runtime `str`; precise in _types.pyi)
    "Alias",
    "Did",
    "NodeId",
    "ObjectId",
    "Oid",
    "RepoId",
    # errors
    "CobError",
    "DatabaseError",
    "Error",
    "GitError",
    "IdentityError",
    "PolicyError",
    "RepositoryError",
    "SignerError",
    "StorageError",
    # identity & profile
    "Home",
    "Profile",
    "Signer",
    # storage & repository (Phase 2)
    "Storage",
    "Repository",
    "HeadInfo",
    "Commit",
    "Tree",
    "TreeEntry",
    "TagInfo",
    "RemoteInfo",
    "ProjectDoc",
    "HistoryPage",
    # validators
    "parse_did",
    "parse_node_id",
    "parse_object_id",
    "parse_oid",
    "parse_repo_id",
    "__version__",
]

# Extension submodule (`radiclepy.radiclepy`); surface its metadata and make
# `__module__` of re-exported objects point at the package, not the extension.
import sys as _sys

_rad = _sys.modules[f"{__name__}.radiclepy"]
__version__ = _rad.__version__
for _name in __all__:
    _obj = vars().get(_name)
    if _obj is not None and getattr(_obj, "__module__", None) == __name__ + ".radiclepy":
        try:
            _obj.__module__ = __name__
        except (AttributeError, TypeError):
            pass
del _rad, _name, _obj, _sys
