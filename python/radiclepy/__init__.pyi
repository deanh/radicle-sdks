from __future__ import annotations

from ._types import Alias, Did, NodeId, ObjectId, Oid, RepoId

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
    # type aliases
    "Alias",
    "Did",
    "NodeId",
    "ObjectId",
    "Oid",
    "RepoId",
]

# ── Exceptions (hierarchy: all inherit from Error) ───────────────────────────

class Error(Exception): ...
class StorageError(Error): ...
class RepositoryError(Error): ...
class GitError(Error): ...
class DatabaseError(Error): ...
class IdentityError(Error): ...
class SignerError(Error): ...
class PolicyError(Error): ...
class CobError(Error): ...

# ── Identifier validators (ValueError on invalid input) ──────────────────────

def parse_repo_id(value: str) -> RepoId: ...
def parse_did(value: str) -> Did: ...
def parse_node_id(value: str) -> NodeId: ...
def parse_oid(value: str) -> Oid: ...
def parse_object_id(value: str) -> ObjectId: ...

# ── Handles ──────────────────────────────────────────────────────────────────

class Home:
    """Filesystem home for a Radicle profile (`RAD_HOME` / `~/.radicle`)."""

    def __init__(self, path: str) -> None: ...
    @classmethod
    def load(cls, path: str) -> Home: ...
    @classmethod
    def resolve(cls) -> Home:
        """Resolve from `RAD_HOME`, else `~/.radicle`."""
        ...
    @property
    def path(self) -> str: ...
    def __repr__(self) -> str: ...

class Aliases:
    """Lookup facade over the profile's alias stores — NOT a collection.

    Upstream only supports `get(node_id)` and `reverse_lookup(alias)`;
    there is no enumeration.
    """

    def get(self, node_id: NodeId) -> str | None: ...
    def reverse_lookup(self, alias: Alias) -> dict[str, list[NodeId]]: ...
    def __repr__(self) -> str: ...

class Signer:
    """Radicle signer: unencrypted keystore, `RAD_PASSPHRASE`, or ssh-agent."""

    @property
    def did(self) -> Did: ...
    @property
    def node_id(self) -> NodeId: ...
    def __repr__(self) -> str: ...

class Profile:
    """A local Radicle profile (keys, config, storage)."""

    @classmethod
    def load(cls) -> Profile:
        """Load from `RAD_HOME` or `~/.radicle`."""
        ...
    @classmethod
    def init(cls, home_path: str, alias: str, passphrase: str | None = None) -> Profile:
        """Initialize a new profile with a **secure random seed** (OS RNG).

        Deterministic seeding is not part of the shipped API (PLAN §5.7):
        `Profile._from_seed` exists only in builds with the `test-helpers`
        cargo feature and is absent from release wheels.
        """
        ...
    @property
    def did(self) -> Did: ...
    @property
    def node_id(self) -> NodeId: ...
    @property
    def home(self) -> Home: ...
    @property
    def aliases(self) -> Aliases: ...
    def signer(self) -> Signer: ...
    def __repr__(self) -> str: ...
