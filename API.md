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
| `ReadOnlyError` | `Error` | writes through read-only COB/policy handles |
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
Write methods raise `ReadOnlyError`.

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

## COB: typed readers & raw inspection (Phase 4)

Read-only. Typed stores materialize upstream `Store<T>` (`ReadOnly`
access); writes are Phase 5. COB `String` fields surface as `str` — the
bytes-first policy applies to byte-typed sources (git), not to UTF-8-by-
construction COB payloads.

#### `Issues`

Read-only typed issue store for one repository.

##### `Issues.open(repository: Repository) -> Issues` (classmethod) — lazy read-only handle; the upstream store is opened at each operation, so repository/store failures occur on use, not at handle construction. `Patches.open` has the same semantics; `open_as` retains a signer and is also lazy.

##### `issues.rid -> RepoId` (attribute) — repository this store is bound to

##### `issues.all() -> list[Issue]` (method) — eager, sorted by entity id; `CobError` on read failure (never swallowed into an empty list)

##### `issues.get(id: ObjectId) -> Issue | None` (method) — `None` if absent; `ValueError` if `id` is not a valid ObjectId

##### `issues.count() -> int` (method) — number of issues

##### `issues.is_empty() -> bool` (method)

#### `Issue` (value class)

Materialized at read time; immutable.

##### `issue.id -> ObjectId` (attribute) — entity id (change-commit OID hex)

##### `issue.title -> str` (attribute)

##### `issue.description -> str` (attribute) — root comment body

##### `issue.author -> Did` (attribute) — `did:key:…`

##### `issue.state -> str` (attribute) — `"open" | "closed"`

##### `issue.close_reason -> str | None` (attribute) — `"unspecified" | "solved"` when closed (upstream `CloseReason` Display)

##### `issue.assignees -> list[Did]` (attribute) — sorted

##### `issue.labels -> list[str]` (attribute) — sorted

##### `issue.comment_count -> int` (attribute)

##### `issue.comment_ids -> list[Oid]` (attribute) — comment ids, store order

##### `issue.comments -> list[Comment]` (attribute) — non-redacted comments in upstream thread order, including the root; copied immutable snapshots.

#### `Patches`

Read-only typed patch store; same surface as `Issues`
(`open`/`rid`/`all`/`get`/`count`/`is_empty`), materializing `Patch`.

#### `Patch` (value class)

Summary fields are materialized from the **latest revision by the patch author** (upstream `latest()`); `revisions` includes non-redacted revisions by all authors.

##### `patch.id -> ObjectId` (attribute)

##### `patch.title -> str` (attribute)

##### `patch.description -> str` (attribute) — latest revision's description

##### `patch.author -> Did` (attribute)

##### `patch.state -> str` (attribute) — `"draft" | "open" | "archived" | "merged"`

##### `patch.target -> str` (attribute) — `"delegates"` or `"branch:<name>"`

##### `patch.base -> str` (attribute) — base commit OID of the latest revision

##### `patch.head -> str` (attribute) — head commit OID of the latest revision

##### `patch.latest_revision -> Oid` (attribute) — ID returned by upstream `latest()`, identifying the patch-author revision described by `base`, `head` and `description`. Use this ID for review/merge of the summary revision, not `revisions[-1].id`.

##### `patch.version -> int` (attribute) — upstream revision-slot count minus one, including every author and redacted slots. **Not an index into `revisions`**, and not the index of `latest_revision` or a document version. `revision_count` counts non-redacted revisions.

##### `patch.labels -> list[str]` (attribute) — sorted

##### `patch.revision_count -> int` (attribute)

##### `patch.revisions -> list[Revision]` (attribute) — non-redacted revisions by all authors, in upstream timeline order. Summary `base`/`head`/`description` use the latest revision by the patch author, which need not be the last revision by another actor.

##### `patch.assignees -> list[Did]` (attribute)

##### `patch.merged_revision -> Oid | None` (attribute)

##### `patch.merged_commit -> Oid | None` (attribute) — both merge fields are set only when evaluated state is `merged`.

#### `Raw`

Raw COB inspection — no domain types required.

##### `Raw.open(repository: Repository) -> Raw` (classmethod)

##### `raw.rid -> RepoId` (attribute)

##### `raw.typenames() -> list[str]` (method) — distinct COB typenames, sorted.
Strict COB-ref acceptance (`cob::TypedId`: `refs/cobs/<typename>/<id>` after
namespace stripping) — COB-shaped non-COB refs (e.g.
`refs/tags/<typename>/<oid>`, which `cob::object::parse_refstr`'s shape check
false-matches) never appear.

##### `raw.entities(typename: str) -> list[ObjectId]` (method) — entity ids with COB refs under `typename`, sorted; same strict acceptance. `ValueError` on invalid typename

##### `raw.refs(typename: str, entity_id: ObjectId) -> list[CobRef]` (method) — COB refs for the entity, sorted by name. These are *not* signed refs: authenticity comes from rad/sigrefs, not from individual COB refs

##### `raw.history(typename: str, entity_id: ObjectId) -> list[RawOp]` (method) — causal history, `entry.parents` traversal only (identity/related links are never followed), deduped across the entity's refs, **topologically ordered** (each op after its loaded parents; deterministic tie-breaks). Insertion order is the contract. Cyclic (impossible-unless-corrupt) data raises `CobError`

#### `RawOp` (value class)

One change entry; one op carries a **non-empty tuple** of actions.

##### `op.id -> Oid` (attribute) — change-commit OID

##### `op.actor -> NodeId` (attribute) — canonical NodeId (upstream stores a PublicKey)

##### `op.timestamp -> int` (attribute) — **unix epoch SECONDS** (upstream `cob::change::Timestamp` is seconds; unlike `node::Timestamp`/routing, which is millis)

##### `op.actions -> tuple[bytes, ...]` (attribute) — raw action payloads (canonical JSON bytes), decoded independently

##### `op.parents -> tuple[Oid, ...]` (attribute) — causal parents (the only traversal edge)

##### `op.related -> tuple[Oid, ...]` (attribute) — related-object links (exposed, never traversed)

##### `op.identity -> Oid | None` (attribute) — identity-COB link (exposed, never traversed)

##### `op.manifest -> CobManifest` (attribute)

##### `op.actions_json() -> tuple[object, ...]` (method) — each action parsed from JSON into plain Python (`None|bool|int|float|str|list|dict`, keys `str`); `CobError` if any action isn't valid JSON

#### `CobManifest` (value class)

##### `manifest.type_name -> str` (attribute) — e.g. `xyz.radicle.issue`

##### `manifest.version -> int` (attribute) — COB schema version

#### `CobRef` (value class)

##### `cob_ref.name -> str` (attribute) — full namespaced ref name, verbatim (`refs/namespaces/<nid>/refs/cobs/…`)

##### `cob_ref.target -> Oid` (attribute) — change commit the ref points at

## Writes (Phase 5)

Narrow, upstream-shaped writes (PLAN §3). The three canonical-ref writes take
**no signer** because upstream takes none; `sign_refs` is the ONLY
signer-taking write. No hidden write-then-sign composition — sequencing is
the caller's explicit choice. For COB stores the signer is fixed at
store-open time (`open_as`, upstream `WriteAs(signer)`); mutating methods
themselves take none.

### Repository writes

##### `repo.set_default_branch_to_canonical_head() -> SetHead` (method) — compute the default-branch head from the delegate set and set it (upstream `set_default_branch_to_canonical_head`)

##### `repo.set_identity_head() -> Oid` (method) — set `rad/id` to the canonical identity head agreed by quorum (upstream `set_identity_head`)

##### `repo.set_remote_identity_root(remote: NodeId) -> Oid` (method) — set the remote's identity-root ref to the repository's canonical identity root. Upstream writes unconditionally (no membership check) and is idempotent. `ValueError` on malformed NodeId

##### `repo.sign_refs(signer: Signer) -> SignedRefs` (method) — sign canonical refs under `refs/rad/sigrefs`; unchanged refs reuse the existing sigrefs commit (no new commit)

#### `SetHead` (value class)

##### `set_head.old -> Oid | None` (attribute) — previous default-branch head (`None` if absent)

##### `set_head.new -> Oid` (attribute) — new default-branch head

##### `set_head.is_updated() -> bool` (method) — whether the head actually moved

#### `SignedRefs` (value class)

##### `signed.node_id -> NodeId` (attribute) — signer / remote namespace

##### `signed.at -> Oid` (attribute) — sigrefs commit carrying this set

##### `signed.parent -> Oid | None` (attribute) — parent sigrefs commit (`None` at root)

##### `signed.feature_level -> str` (attribute) — `"none" | "root" | "parent"` (upstream `FeatureLevel` has exactly these three in 0.25.1)

##### `signed.refs() -> dict[str, Oid]` (method) — the signed set (sorted). The cryptographic signature itself is not surfaced (verified/re-derived upstream on read)

### Typed COB writes

Open a store with a signer once; mutations carry no signer argument:

```python
issues = rad.Issues.open_as(repo, profile.signer())   # write mode
m = issues.create("Title", "desc", labels=["bug"])
m.comment("looks good")                                # reply_to defaults to root
m.lifecycle("closed", "solved")
patches = rad.Patches.open_as(repo, profile.signer())
pm = patches.draft("WIP", "d", base=base, head=head, target="branch:release")
```

Stores opened with plain `open()` raise `ReadOnlyError` on write attempts.

#### `Issues` (write extensions)

##### `Issues.open_as(repository: Repository, signer: Signer) -> Issues` (classmethod) — write store (upstream `WriteAs(signer)`)

##### `issues.create(title: str, description: str, labels: Sequence[str] | None = None, assignees: Sequence[Did] | None = None) -> IssueMut` (method) — creates and persists immediately; `ValueError` on invalid title/labels/DIDs

##### `issues.get_mut(id: ObjectId) -> IssueMut` (method) — write stores only; `CobError` if absent

#### `IssueMut` (value class)

Each mutator commits **one COB transaction** and returns the new change-entry
`Oid`; the `issue` snapshot refreshes after each call.

##### `issue_mut.id -> ObjectId` (attribute)

##### `issue_mut.issue -> Issue` (attribute) — snapshot after the latest change through this handle

##### `issue_mut.reload() -> Issue` (method) — re-read from storage

##### `issue_mut.comment(body: str, reply_to: Oid | None = None) -> Oid` (method) — `reply_to` defaults to the root comment (upstream requires a reply target)

##### `issue_mut.edit(title: str) -> Oid` (method)

##### `issue_mut.edit_description(description: str) -> Oid` (method) — edits the root comment (upstream semantics)

##### `issue_mut.label(labels: Sequence[str]) -> Oid` (method) — **sets** the label set: upstream `Action::Label` REPLACES (not additive)

##### `issue_mut.assign(assignees: Sequence[Did]) -> Oid` (method) — **sets** the assignee set: upstream `Action::Assign` REPLACES (like `label`); an empty list clears

##### `issue_mut.lifecycle(state: str, reason: str | None = None) -> Oid` (method) — `state` `"open"|"closed"`; `reason` (closing only) `"unspecified"` (default) | `"solved"`; invalid combinations raise `ValueError`

#### `Patches` (write extensions)

**Creating or updating patches from a working copy: use Git, not this API.**
Use these separate commands from a checkout (requires `git-remote-rad`):

```sh
# Open a new patch.
git push rad HEAD:refs/patches
# Add a revision to an existing patch (replace <id> with its patch ID).
git push rad HEAD:refs/heads/patches/<id>
```

The remote helper copies the commits into storage, derives `base`, creates or
updates the patch through the same upstream API this binding wraps, and signs
refs.

`create`/`draft`/`update` are for programs whose commits are **already in
storage**. They write the patch object directly. For **creation**, the paths
differ as follows:

| Step | `git push rad HEAD:refs/patches` | `create`/`draft` |
|---|---|---|
| Get commits into storage | yes | caller's responsibility |
| `base` | merge-base with the canonical head by default; `patch.base` push option overrides it | must be passed explicitly |
| Empty patch (`base == head`) | rejected | accepted (upstream API allows it) |
| `refs/heads/patches/<id>` branch | created | not created |
| Sign refs | yes | yes (COB writes sign) |
| Network sync | attempted when seeding and the node is running, unless `no-sync`; sync failure does not fail the local push | never |

For **updates**, pushing to `refs/heads/patches/<id>` uses the same default
base calculation (or `patch.base` override), skips an already-recorded
base/head pair, and can record an open patch as merged when `base == head`.
SDK `update()` instead takes an explicit base, adds a revision without moving
the patch branch, and does not automatically record a merge or sync. Its
commits must already be in storage, and the COB transaction signs refs.

The missing patch branch doesn't affect replication: revisions record `base`
and `head` as related commits of the patch's change entry, so they travel with
the patch. Don't import commits by pushing to your namespaced **default
branch** (`refs/namespaces/<nid>/refs/heads/<default>`): that moves your copy
of the project's branch, which feeds canonical-head and merge evaluation.

##### `Patches.open_as(repository: Repository, signer: Signer) -> Patches` (classmethod)

##### `patches.create(title, description, *, base, head, target="delegates", labels=None) -> PatchMut` (method) — open patch; `target` `"delegates"` or `"branch:<name>"` (bare name accepted; a fully-qualified `branch:refs/heads/<name>` is accepted too and **canonicalizes to bare output**); `base`/`head` git commit Oids that exist in the repository

##### `patches.draft(...) -> PatchMut` (method) — same signature as `create`, created in `draft` state.

##### `patches.get_mut(id: ObjectId) -> PatchMut` (method) — write stores only; `ValueError` for malformed IDs, `ReadOnlyError` for read-only access, `CobError` if absent.

#### `PatchMut` (mutable handle)

Reopens and reloads on every call; a signer is retained from the store.
Each mutation is one upstream COB transaction and signs refs upstream.
Authorization/store failures raise `CobError`; validator failures raise `ValueError`.
No atomic multi-call transaction or automatic Git branch movement is promised.

##### `patch_mut.id -> ObjectId` (attribute)

##### `patch_mut.patch -> Patch` (attribute) — snapshot after the latest operation through this handle

##### `patch_mut.reload() -> Patch` (method) — refresh from storage, including other writers' changes

##### `patch_mut.edit(title: str, target: str) -> Oid` (method)

##### `patch_mut.label(labels: Sequence[str]) -> Oid` (method) — replaces the label set

##### `patch_mut.assign(assignees: Sequence[Did]) -> Oid` (method) — replaces the assignee set

##### `patch_mut.update(description: str, *, base: Oid, head: Oid) -> Oid` (method) — add a revision; commits must exist in storage, returns revision ID

##### `patch_mut.ready() -> bool` (method) — false unless draft

##### `patch_mut.unready() -> bool` (method) — false unless open with no merge conflicts

##### `patch_mut.archive() -> bool` (method) — upstream always attempts lifecycle and returns true on success

##### `patch_mut.unarchive() -> bool` (method) — false unless archived

##### `patch_mut.comment(revision: Oid, body: str, reply_to: Oid | None = None) -> Oid` (method) — revision discussion; no inline location or embeds

##### `patch_mut.review(revision: Oid, verdict: str | None = None, summary: str | None = None) -> Oid` (method) — verdict `accept`/`reject`; at least one of verdict/summary must be supplied (`CobError` otherwise); returns review ID

##### `patch_mut.merge(revision: Oid, commit: Oid) -> Oid` (method) — returns operation ID, **not a Git merge**. Does not move branches or clean up refs. Upstream checks authorization and evaluates whether the commit is in the author's target branch and the delegate quorum is met. An operation may succeed but leave the patch open; inspect/reload `patch.state`. Commit must already exist in storage.

Note: `Patch.target` surfaces `"delegates"` or `"branch:<name>"` with exactly
one guaranteed `refs/heads/` prefix removed (`strip_prefix` — `repeat`-trim
would eat repeated prefixes). Output is canonical *bare*: qualified input
`branch:refs/heads/foo` reads back as `branch:foo`. Degenerate branches whose
bare name itself starts with `refs/heads/` share the canonical short form and
are the only non-round-tripping case.

## Local project lifecycle (alpha)

These are **non-interactive crate adapters**, not calls to the interactive `rad init` CLI. All required metadata is explicit. No node daemon or `rad` executable is needed. Upstream init/checkout invoke the **Git executable** (`git` on PATH). These synchronous operations retain the GIL, including during upstream subprocess calls.

#### `init_project(storage: Storage, signer: Signer, path: str, *, name: str, description: str, default_branch: str, visibility: str = "public") -> RepoId`

Convert an existing Git repository with a committed, short-named default branch into a Radicle project. Visibility is `public` or `private` (empty explicit allow list; advanced access administration is excluded). Sets up the `rad` remote and local storage, and signs refs. Returns the new project ID. Binding name/branch/visibility validation raises `ValueError`; absent repository/branch/commit raises `GitError`; upstream project-payload and multi-step failures raise `Error`. An existing `rad` remote is refused before upstream init (`Error`), preserving its configuration and tracking refs; init is not a metadata-update API. For new initialization, cleanup follows upstream and is not a rollback guarantee.

#### `fork_project(storage: Storage, signer: Signer, repo_id: RepoId) -> None`

Create/overwrite the signer's namespaced default branch from the stored canonical head, then sign refs. The project must already be in storage. `ValueError` for malformed ID, `Error` for upstream failures.

#### `checkout_project(storage: Storage, repo_id: RepoId, remote: NodeId, path: str, *, bare: bool = False) -> str`

Create a working copy from **local storage**, fetching canonical `refs/heads/*` and checking out the canonical default-branch head. `remote` selects the namespace in the configured **push URL**, not the source of the checked-out content; a different peer branch is not checked out. Preserve upstream bare-mode behavior and return the supplied path. This is not network clone. An existing repository destination or absent stored project fails. `ValueError` for malformed IDs, `Error` for upstream failures. A failed checkout may leave a partial destination directory; callers own cleanup. Later external Git operations on the configured Radicle remote need `git-remote-rad` installed separately.

## Collaboration snapshot values

#### `Comment`

Read-only properties: `id: Oid`, `author: Did`, `body: str` (latest edit), `timestamp: int` (original creation, epoch **milliseconds**), `reply_to: Oid | None`. Redacted comments are omitted upstream. This narrow snapshot does not expose edit histories, reactions, embeds or inline locations.

#### `Review`

Read-only properties: `id: Oid`, `author: Did`, `verdict: Literal['accept', 'reject'] | None`, `summary: str` (latest edit), `timestamp: int` (epoch **milliseconds**). Revision reviews contain one current review per actor, in upstream actor-key order; inline review discussions are not exposed.

#### `Revision`

Read-only properties: `id: Oid`, `author: Did`, `description: str`, `base: Oid`, `head: Oid`, `timestamp: int` (epoch **milliseconds**), `comments: list[Comment]` (non-redacted, thread order), `reviews: list[Review]`.

These typed timestamps come from `cob::common::Timestamp`; **RawOp.timestamp remains epoch seconds** from the raw change entry. Collection getters return copies; snapshots do not refresh automatically. Signers and mutable stores/handles are thread-affine; no async/thread-safety guarantee is made.

## Quick start

See [README.md](./README.md) for a complete runnable, non-interactive temporary-project example and Git-URI installation.

## Keyword-only metadata and read-only errors

Project metadata after `path` is keyword-only. Patch creation/drafting accepts
`title, description, *, base, head, target="delegates", labels=None`;
patch updates accept `description, *, base, head`. Legacy positional forms
raise `TypeError`. All writes through read-only COB/policy handles raise
`radiclepy.ReadOnlyError`, a direct subclass of `Error`. Input-validation
errors remain distinct; no generalized missing-object exception was added.

## Snapshot equality and hashing

All immutable value classes have structural equality and hashing over every
stored field, recursively including nested snapshots and ordered collections.
Equal independently loaded snapshots collapse into one set/dictionary key.
Snapshots with the same entity ID but different contents are unequal: use
`.id`/`.oid` explicitly for identity-only comparisons. Hashing large snapshots
is proportional to their contents. Hashes use a shared, randomly keyed Rust
hasher per process, including for nested content; they are independent of
`PYTHONHASHSEED` and are not a persistent wire format. Equal snapshots still
have equal hashes within the process; equality does not depend on the key.

Collection getters return copies, so mutating those copies never changes the
snapshot or its hash. Handles (`Profile`, `Home`, `Storage`, `Repository`,
`Signer`, stores, `Raw`, `*Mut`, aliases) retain identity equality/hashing.

## Python convenience surface

All public filesystem inputs (`path` and `home_path`) accept
`str | os.PathLike[str]`, including custom `__fspath__` implementations.
Bytes and `PathLike[bytes]` are rejected with `TypeError`. Path returns remain
strings; existing non-UTF-8 path-property errors are unchanged (no lossy
conversion is introduced).

- `profile.storage -> Storage` is equivalent to `Storage.from_profile(profile)`.
- `rid in storage` delegates to `storage.contains(rid)`, including its errors
  and filesystem access. It does not make storage iterable or change truthiness.
- `iter(tree)` iterates a copied entry list in canonical Git order; it performs
  no I/O and holds no repository handle open.
- `issue_mut.close(reason="unspecified") -> Oid` and
  `issue_mut.reopen() -> Oid` each perform exactly one upstream lifecycle write.
  The original `lifecycle` API remains available.
- Collaboration reprs show bounded IDs/metadata, not discussion bodies.
- Stubs narrow finite states/kinds/policies/scopes to `Literal` annotations.
  Runtime validation is unchanged; dynamically typed `str` callers may need
  explicit narrowing for static type checking. Patch targets remain `str`
  because `branch:<name>` is an open-ended domain.

## Layout notes

- Extension module: `radiclepy.radiclepy` (compiled); the package
  `radiclepy/__init__.py` re-exports the public names.
- Stubs: `__init__.pyi` + `_types.pyi` (identifier aliases), `py.typed`.
