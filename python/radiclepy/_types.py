"""Runtime identifier aliases (PLAN §5.1).

Identifiers are strings at the boundary. These aliases exist for typed
callers and documentation; they are ``str`` at runtime. The paired
``_types.pyi`` carries the precise ``TypeAlias`` declarations for static
analysis.
"""

from __future__ import annotations

#: Canonical repository id, e.g. ``"rad:z…"``.
RepoId = str

#: W3C DID key form, e.g. ``"did:key:z6Mk…"``.
Did = str

#: Node id / public key, e.g. ``"z6Mk…"``.
NodeId = str

#: Git object id, lowercase 40-char hex.
Oid = str

#: COB object id, lowercase 40-char hex.
ObjectId = str

#: Node alias, e.g. ``"alice"``.
Alias = str

__all__ = ["RepoId", "Did", "NodeId", "Oid", "ObjectId", "Alias"]
