"""Identifier type aliases (PLAN §5.1).

Identifiers are strings at the boundary; these aliases exist for
documentation and typed callers, not as a runtime class hierarchy.
"""

from __future__ import annotations

from typing import TypeAlias

#: Canonical repository id, e.g. ``"rad:z…"``.
RepoId: TypeAlias = str

#: W3C DID key form, e.g. ``"did:key:z6Mk…"``.
Did: TypeAlias = str

#: Node id / public key, e.g. ``"z6Mk…"``.
NodeId: TypeAlias = str

#: Git object id, lowercase 40-char hex.
Oid: TypeAlias = str

#: COB object id, lowercase 40-char hex.
ObjectId: TypeAlias = str

#: Node alias, e.g. ``"alice"``.
Alias: TypeAlias = str
