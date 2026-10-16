"""Identifier type aliases.

Identifiers are strings at the boundary; these aliases exist for
documentation and typed callers, not as a runtime class hierarchy.
"""

from __future__ import annotations

__all__ = ["RepoId", "Did", "NodeId", "Oid", "ObjectId", "Alias"]

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
