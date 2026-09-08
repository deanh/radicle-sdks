# `radiclepy` Python API

Status: pre-alpha, Phase 1. This document is the contract: every public
name, its kind (class / attribute / method / function), and its failure
modes. `tests/test_api.py` checks that documented names exist.

Identifier convention (plan §5.1): identifiers are **strings** at the
boundary; stubs expose aliases (`RepoId`, `Did`, `NodeId`, `Oid`,
`ObjectId`, `Alias`) via `radiclepy._types` for typed callers.

Error convention (plan §5.8): our own syntactic validators raise
`ValueError`; operational failures raise subclasses of `radiclepy.Error`.

---

## Module: `radiclepy`

### Functions

#### `parse_repo_id(value: str) -> RepoId`
Validate a repository id (`rad:z…`); return the canonical string.
Raises `ValueError` if syntactically invalid.

#### `parse_did(value: str) -> Did`
Validate a DID (`did:key:z…`); return the canonical string.
Raises `ValueError`.

#### `parse_node_id(value: str) -> NodeId`
Validate a node id / public key (`z6Mk…`); return the canonical string.
Raises `ValueError`.

#### `parse_oid(value: str) -> Oid`
Validate a git OID (40-char hex); return lowercase canonical form.
Raises `ValueError`.

#### `parse_object_id(value: str) -> ObjectId`
Validate a COB object id (40-char hex); return canonical form.
Raises `ValueError`.

### Exceptions

| Name | Parent | Raised on |
|---|---|---|
| `Error` | `Exception` | base; profile load/init and io failures today |
| `StorageError` | `Error` | (registered; used from Phase 2) |
| `RepositoryError` | `Error` | (registered; used from Phase 2) |
| `GitError` | `Error` | (registered; used from Phase 2) |
| `DatabaseError` | `Error` | (registered; used from Phase 3) |
| `IdentityError` | `Error` | (registered; used from Phase 2) |
| `SignerError` | `Error` | `Profile.signer()` failures |
| `PolicyError` | `Error` | (registered; used from Phase 3) |
| `CobError` | `Error` | (registered; used from Phase 4) |

All are catchable as `radiclepy.Error`.

### Classes

#### `Profile`

A local Radicle profile: keys, config, storage.

##### `Profile.load() -> Profile` (classmethod)
Load the local profile from `RAD_HOME` or `~/.radicle`.
Raises `radiclepy.Error` if absent/unreadable.

##### `Profile.init(home_path: str, alias: str, passphrase: str | None = None) -> Profile` (classmethod)
Initialize a new profile at `home_path`. The key seed comes from a
**secure random fill (OS RNG, 32 bytes)** — there is no public way to
inject a seed (plan §5.7). `passphrase` encrypts the keystore when given.
The alias is validated **before** any filesystem state is created.
Raises `ValueError` for invalid aliases; `radiclepy.Error` for init
failures (including OS RNG unavailability — operational, not input).

##### Deterministic seeding — NOT part of the shipped API (plan §5.7)
`Profile._from_seed(home_path, alias, seed, passphrase=None)` exists only
in builds compiled with the `test-helpers` cargo feature (`maturin
develop --features test-helpers`) and is **absent from release wheels**.
Used exclusively by the test fixtures. `test_api.py` treats it as
non-public; CI's wheel smoke step asserts its absence.

##### `profile.did -> Did` (attribute)
Profile DID, `did:key:z…`.

##### `profile.node_id -> NodeId` (attribute)
Profile node id / public key, `z6Mk…`.

##### `profile.home -> Home` (attribute)
The filesystem home backing this profile.

##### `profile.aliases -> Aliases` (attribute)
Alias **lookup facade** — not a collection (upstream cannot enumerate).

##### `profile.signer() -> Signer` (method)
Load the signer: unencrypted keystore, `RAD_PASSPHRASE`, or ssh-agent
(upstream `radicle::Profile::signer()` semantics).
Raises `SignerError`.

##### `repr(profile) -> str`
`Profile(<did>)`.

#### Type aliases (runtime attributes; all are `str`)

Documented and runtime-importable, precise `TypeAlias` declarations in
`radiclepy._types` (which is also runtime-importable):

`radiclepy.RepoId`, `radiclepy.Did`, `radiclepy.NodeId`, `radiclepy.Oid`,
`radiclepy.ObjectId`, `radiclepy.Alias`.

#### `Home`

Filesystem home (`RAD_HOME` / `~/.radicle` resolution).

##### `Home(path: str)` (constructor)
Create/ensure a home at `path` (canonicalized). Raises `ValueError` for
non-UTF-8 paths, `radiclepy.Error` on io failures.

##### `Home.load(path: str) -> Home` (classmethod)
Load an existing home from `path`. Raises `radiclepy.Error`.

##### `Home.resolve() -> Home` (classmethod)
Resolve from the environment: `RAD_HOME`, else `~/.radicle`.
Raises `radiclepy.Error`.

##### `home.path -> str` (attribute)
Canonical filesystem path.

#### `Aliases`

Lookup facade (plan §3) — deliberately **not** a collection.

##### `aliases.get(node_id: NodeId) -> str | None` (method)
Alias for `node_id`, if known. Raises `ValueError` for invalid node ids.

##### `aliases.reverse_lookup(alias: str) -> dict[str, list[NodeId]]` (method)
All aliases matching `alias` → their node ids. Raises `ValueError` for
invalid alias strings.

#### `Signer`

Radicle signing device (keystore key or ssh-agent).

##### `signer.did -> Did` (attribute)
DID for this signer's key.

##### `signer.node_id -> NodeId` (attribute)
Node id / public key for this signer.

---

### Storage & repository (Phase 2)

#### `Storage`

Repository storage for a radicle home (upstream `radicle::Storage`).

##### `Storage.from_profile(profile: Profile) -> Storage` (classmethod)
Primary constructor; the storage of `profile` (git identity comes from it).

##### `Storage.open(path: str, alias: str, node_id: NodeId) -> Storage` (classmethod)
Low-level constructor: open/create storage at `path` with an explicit git
user identity. Raises `ValueError` for invalid `alias`/`node_id`,
`StorageError` for operational failures.

##### `storage.path -> str` (attribute)
Filesystem path of this storage.

##### `storage.repository(rid: RepoId) -> Repository` (method)
Open a repository for **reading**. Raises `RepositoryError` if absent,
`ValueError` for a malformed `rid`.

##### `storage.contains(rid: RepoId) -> bool` (method)
Whether the repository exists in this storage.

##### `storage.repositories() -> list[RepoId]` (method)
All repository ids in this storage (unsorted).

#### `Repository`

A **read-only handle**: reopens the underlying storage per operation, so it
holds no live git handles and is safe to pass between threads. Mutations
arrive only via explicit write APIs in later phases.

##### `repository.rid -> RepoId` (attribute)
Repository id.

##### `repository.head -> HeadInfo` (attribute)
Head (`refs/heads/...`) and its OID.

##### `repository.identity() -> ProjectDoc` (method)
Verified canonical identity document. Raises `IdentityError`.

##### `repository.description() -> str` (method)
Project description (from the identity document). The payload requires a
description, so failures (corruption, verification, I/O) propagate as
`IdentityError` — there is no `None` sentinel.

##### `repository.branches() -> list[str]` (method)
Branch short names (sorted).

##### `repository.branch_head(name: str) -> Oid | None` (method)
OID a branch points at, or `None` **only when the branch does not exist**
(`NotFound`). Raises `ValueError` for binding-invalid names, `GitError` for
any other libgit2 failure.

##### `repository.tags() -> list[TagInfo]` (method)
Tags (sorted by short name) with raw/peeled distinction.

##### `repository.rev_parse(rev: str) -> Oid` (method)
Resolve a git revision expression. Raises `GitError` for **invalid or
unresolved** revisions (revspec semantics are git's — `ValueError` is
reserved for our `parse_*` validators).

##### `repository.commit(oid: Oid) -> Commit` (method)
Load a commit. Raises `GitError` if absent or not a commit.

##### `repository.tree(oid: Oid) -> Tree` (method)
Load a tree. Raises `GitError` if absent or not a tree.

##### `repository.blob(oid: Oid) -> bytes` (method)
Raw blob content. Raises `GitError` if absent or not a blob.

##### `repository.history(rev: str, *, limit: int = 100, offset: int = 0) -> HistoryPage` (method)
Bounded commit-history page. Binding-validated pagination (`ValueError`):
`1 <= limit <= 1000` (larger limits clamped to 1000; `limit < 1` raises,
including negatives — never silently adjusted) and `offset >= 0`.
Ordering: libgit2 revwalk default order (the pinned upstream exposes no
explicit sorting — see FINDINGS-02 S7).

##### `repository.remotes() -> list[RemoteInfo]` (method)
Remotes tracked by this repository (sorted by fetcher).

#### Value classes (immutable)

Bytes-first fields; `*_str` accessors are explicitly lossy (`U+FFFD` may
appear). OIDs are lowercase 40-char hex strings.

#### `HeadInfo`
##### `head_info.name -> str` (attribute) — qualified ref, e.g. `refs/heads/master`.
##### `head_info.oid -> Oid` (attribute)

#### `Commit`
##### `commit.oid -> Oid` (attribute)
##### `commit.tree -> Oid` (attribute) — root tree.
##### `commit.parents -> list[Oid]` (attribute)
##### `commit.author_name -> bytes` (attribute)
##### `commit.author_name_str -> str` (attribute) — lossy
##### `commit.author_email -> bytes` (attribute)
##### `commit.author_email_str -> str` (attribute) — lossy
##### `commit.author_time -> int` (attribute) — unix epoch seconds
##### `commit.author_offset_min -> int` (attribute) — tz offset, minutes
##### `commit.committer_name -> bytes` (attribute)
##### `commit.committer_name_str -> str` (attribute) — lossy
##### `commit.committer_email -> bytes` (attribute)
##### `commit.committer_email_str -> str` (attribute) — lossy
##### `commit.committer_time -> int` (attribute)
##### `commit.committer_offset_min -> int` (attribute)
##### `commit.message -> bytes` (attribute) — raw message bytes
##### `commit.message_str -> str` (attribute) — lossy
##### `commit.encoding -> str | None` (attribute) — message encoding header

#### `Tree` / `TreeEntry`
Ordering contract: `tree.entries` is in **canonical git tree order**
(libgit2 walk order; directories sort as `<name>/`). Never re-sorted.

##### `tree.oid -> Oid` (attribute)
##### `tree.entries -> list[TreeEntry]` (attribute)
##### `len(tree) -> int`
##### `entry.name -> bytes` (attribute) — may be invalid UTF-8
##### `entry.name_str -> str` (attribute) — lossy
##### `entry.kind -> str | None` (attribute) — `"blob"|"tree"|"commit"` (symlinks are `"blob"` with mode `0o120000`)
##### `entry.mode -> int` (attribute) — raw unix filemode (e.g. `0o100644`)
##### `entry.oid -> Oid` (attribute)

#### `TagInfo`
Peeling is performed **only on tag objects**; for lightweight tags the
target *is* the peeled object (`peeled == target`). "Peeled" = fully
resolved through any tag chain, not commit-specific.

##### `tag.name -> str` (attribute) — short name
##### `tag.target -> Oid` (attribute) — ref target (tag object if annotated)
##### `tag.kind -> str` (attribute) — `"annotated" | "lightweight"`
##### `tag.peeled -> Oid` (attribute)
##### `tag.peeled_kind -> str` (attribute) — `"commit"|"tree"|"blob"` (a fully-peeled target is never a tag object)

#### `RemoteInfo`
##### `remote.fetcher -> NodeId` (attribute)
##### `remote.heads -> list[str]` (attribute) — signed ref names

#### `ProjectDoc`
Canonical project identity document (verified at load).

##### `doc.name -> str` (attribute)
##### `doc.description -> str` (attribute)
##### `doc.default_branch -> str` (attribute) — short name from payload
##### `doc.version -> int` (attribute) — identity document version
##### `doc.delegates -> list[Did]` (attribute)
##### `doc.threshold -> int` (attribute)
##### `doc.visibility -> str` (attribute) — `"public" | "private"`
##### `doc.commit -> Oid` (attribute) — identity commit
##### `doc.blob -> Oid` (attribute) — identity document blob

#### `HistoryPage`
##### `page.name -> str` (attribute) — revision expression as passed to `history()` (not normalized)
##### `page.head -> Oid` (attribute) — resolved walk root
##### `page.oids -> list[Oid]` (attribute) — this page (walk order)
##### `page.offset -> int` (attribute) — entries skipped before this page
##### `page.has_more -> bool` (attribute)

---

### Policy & routing (Phase 3)

#### `PolicyStore`

Follow/seed policy store (upstream `node::policy::Store`). Own sqlite file
(`<home>/node/policies.db`). The binding mirrors the **raw upstream store
surface** — no implicit file-config fallbacks (see FINDINGS-03).
Identifier/policy/scope validation happens binding-side (`ValueError`)
before any database call. All write methods return `bool`: `true` when the
store actually changed.

##### `PolicyStore.from_profile(profile: Profile) -> PolicyStore` (classmethod)
Primary constructor (read-write). Also available as `profile.policy_store()`.

##### `PolicyStore.reader(profile: Profile) -> PolicyStore` (classmethod)
Read-only handle (upstream `Store::reader`: concurrent readers, no locking).
Write methods raise `StorageError`.

##### `PolicyStore.open(path: str) -> PolicyStore` (classmethod)
Low-level read-write constructor at an explicit path.

##### `policy.path -> str` (attribute)
##### `policy.follow(node_id: NodeId, *, alias: str | None = None) -> bool` (method)
##### `policy.unfollow(node_id: NodeId) -> bool` (method)
##### `policy.seed(rid: RepoId, *, scope: str = "all") -> bool` (method) — scope: `"all" | "followed"`
##### `policy.unseed(rid: RepoId) -> bool` (method)
##### `policy.set_follow_policy(node_id: NodeId, policy: str) -> bool` (method) — `"allow" | "block"`
##### `policy.set_seed_policy(rid: RepoId, policy: str) -> bool` (method)
##### `policy.unblock_rid(rid: RepoId) -> bool` (method)
##### `policy.unblock_nid(node_id: NodeId) -> bool` (method)
##### `policy.is_following(node_id: NodeId) -> bool` (method)
##### `policy.is_seeding(rid: RepoId) -> bool` (method)
##### `policy.is_blocked(node_id: NodeId) -> bool` (method) — explicit block, distinct from unfollowed
##### `policy.follow_policy(node_id: NodeId) -> FollowPolicyInfo | None` (method)
##### `policy.seed_policy(rid: RepoId) -> SeedPolicyInfo | None` (method)
##### `policy.follow_policies() -> list[FollowPolicyInfo]` (method) — eager
##### `policy.seed_policies() -> list[SeedPolicyInfo]` (method) — eager
##### `policy.nodes_by_alias(alias: str) -> list[tuple[NodeId, str]]` (method)
**Fuzzy** (substring, case-insensitive) match — upstream `LIKE`; not exact.

#### `RoutingStore`

Read-only routing table (upstream `node::routing::Store`, physically the
node database). Raises `DatabaseError` if the node database does **not
exist** (the read-only open creates nothing; freshly-initialized profiles do
have a migrated `node.db`, so they open with an empty table). Timestamps are
**unix epoch milliseconds** (upstream `node::Timestamp` is a millis `u64`
newtype). Inventory mutations are writes — Phase 5.

##### `RoutingStore.from_profile(profile: Profile) -> RoutingStore` (classmethod)
Also `profile.routing_store()`.

##### `routing.get(rid: RepoId) -> list[NodeId]` (method) — sorted
##### `routing.get_inventory(node_id: NodeId) -> list[RepoId]` (method) — sorted
##### `routing.entry(rid: RepoId, node_id: NodeId) -> int | None` (method) — last-seen millis
##### `routing.entries() -> list[RoutingEntry]` (method) — eager; per-entry timestamp lookup (N+1)
##### `routing.count(rid: RepoId) -> int` (method)
##### `routing.total() -> int` (method)
##### `routing.is_empty() -> bool` (method)

#### Value classes (Phase 3)

#### `FollowPolicyInfo`
##### `follow_policy_info.node_id -> NodeId` (attribute)
##### `follow_policy_info.alias -> str | None` (attribute)
##### `follow_policy_info.policy -> str` (attribute) — `"allow" | "block"`

#### `SeedPolicyInfo`
##### `seed_policy_info.rid -> RepoId` (attribute)
##### `seed_policy_info.policy -> str` (attribute) — `"allow" | "block"`
##### `seed_policy_info.scope -> str | None` (attribute) — `"all" | "followed"`; `None` iff blocked

#### `RoutingEntry`
##### `routing_entry.rid -> RepoId` (attribute)
##### `routing_entry.node_id -> NodeId` (attribute)
##### `routing_entry.timestamp -> int | None` (attribute) — epoch milliseconds

---

## Quick start

```python
import radiclepy as rad

profile = rad.Profile.init("/tmp/rad-demo", "alice")   # secure random key
storage = rad.Storage.from_profile(profile)
repo = storage.repository("rad:z…")                     # read-only handle
head = repo.head
commit = repo.commit(head.oid)
print(head.name, commit.oid, commit.message_str)

for entry in repo.tree(commit.tree).entries:            # canonical git order
    print(entry.name_str, entry.kind, oct(entry.mode))

policy = profile.policy_store()                         # Phase 3
policy.follow(profile.node_id, alias="self")
print(policy.follow_policies())
```

## Layout notes

- Extension module: `radiclepy.radiclepy` (compiled); the package
  `radiclepy/__init__.py` re-exports the public names.
- Stubs: `__init__.pyi` + `_types.pyi` (identifier aliases), `py.typed`.
