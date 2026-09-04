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

##### `Profile.init(home_path: str, alias: str, passphrase: str | None = None) -> Profile` (staticmethod)
Initialize a new profile at `home_path`. The key seed comes from a
**secure random fill (OS RNG, 32 bytes)** — there is no public way to
inject a seed (plan §5.7). `passphrase` encrypts the keystore when given.
Raises `ValueError` for invalid aliases; `radiclepy.Error` for init
failures.

##### `Profile._from_seed(home_path: str, alias: str, seed: bytes, passphrase: str | None = None) -> Profile` (staticmethod, **private / test-only**)
Deterministic init from an exact 32-byte seed. Unstable; used only by the
test fixtures; **never for production**. Raises `ValueError` if
`len(seed) != 32` or on bad alias.

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

## Quick start

```python
import radiclepy as rad

profile = rad.Profile.init("/tmp/rad-demo", "alice")   # secure random key
signer = profile.signer()
print(profile.did, profile.node_id, signer.node_id)

rad.parse_did(profile.did)     # -> canonical Did (str)
rad.parse_oid("0" * 40)        # -> canonical Oid (str)
```

## Layout notes

- Extension module: `radiclepy.radiclepy` (compiled); the package
  `radiclepy/__init__.py` re-exports the public names.
- Stubs: `__init__.pyi` + `_types.pyi` (identifier aliases), `py.typed`.
