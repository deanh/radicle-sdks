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
    CobManifest,
    CobRef,
    Commit,
    Comment,
    Revision,
    Review,
    init_project,
    fork_project,
    checkout_project,
    DatabaseError,
    Error,
    GitError,
    HeadInfo,
    HistoryPage,
    Home,
    IdentityError,
    Issue,
    IssueMut,
    Issues,
    PolicyError,
    FollowPolicyInfo,
    PolicyStore,
    Profile,
    ProjectDoc,
    Patch,
    PatchMut,
    Patches,
    Raw,
    RawOp,
    RemoteInfo,
    RoutingEntry,
    RoutingStore,
    SeedPolicyInfo,
    SetHead,
    SignedRefs,
    Repository,
    RepositoryError,
    ReadOnlyError,
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
    "ReadOnlyError",
    "SignerError",
    "StorageError",
    # identity & profile
    "Home",
    "Profile",
    "Signer",
    # storage & repository
    "Storage",
    "Repository",
    "HeadInfo",
    "Commit",
    "Comment",
    "Revision",
    "Review",
    "init_project",
    "fork_project",
    "checkout_project",
    "Tree",
    "TreeEntry",
    "TagInfo",
    "RemoteInfo",
    "ProjectDoc",
    "HistoryPage",
    # COB: typed + raw
    "Issues",
    "Issue",
    "IssueMut",
    "Patches",
    "Patch",
    "PatchMut",
    "Raw",
    "RawOp",
    "CobRef",
    "CobManifest",
    # writes
    "SetHead",
    "SignedRefs",
    # policy & routing
    "PolicyStore",
    "RoutingStore",
    "FollowPolicyInfo",
    "SeedPolicyInfo",
    "RoutingEntry",
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
