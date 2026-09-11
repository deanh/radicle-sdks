# Changelog

All notable changes to `radicle-py` are documented here.

## 0.1.0 — first release

Bindings scoped exclusively to the `radicle` crate `=0.25.1` (crates.io
source authoritative). abi3 wheels (CPython ≥ 3.9), typed (PEP 561),
no async runtime, no p2p networking.

### Identity & profiles
- `Profile.load` / `Profile.init` (secure random key), `Home`, `Aliases`,
  `Signer` (`did:key:…` DID, canonical NodeId).
- Identifier aliases as `str` at the boundary (`RepoId`, `Did`, `NodeId`,
  `Oid`, `ObjectId`, `Alias`) with strict `parse_*` validators.

### Storage & repositories (read-only)
- `Storage.from_profile` / `Storage.open`; `Repository` handles.
- `head`, `identity` (verified `ProjectDoc`), `description`,
  `branches` / `branch_head`, `tags` (peel semantics documented),
  `rev_parse`, `commit` / `tree` / `blob`, paged `history(limit, offset)`,
  `remotes`. Value classes with explicit bytes-first UTF-8 policy
  (`name`/`message` bytes + lossy `*_str` accessors).

### Policy & routing stores
- `PolicyStore` (own sqlite file, follow/seed policies, block states,
  fuzzy `nodes_by_alias`) and read-only `RoutingStore`
  (millisecond timestamps, inventory entries).

### COB
- Typed readers: `Issues` / `Patches` materializing `Issue` / `Patch`
  (states, labels, comments; patch revisions).
- Raw inspection: `Raw.typenames` / `entities` / `refs` / `history` —
  strict `refs/cobs/…` acceptance, causal-only (`entry.parents`) walks,
  deterministic topological order, cycles raise `CobError`.
- `RawOp`: epoch-**seconds** timestamp, tuple of raw action bytes,
  `CobManifest`, `actions_json()`.

### Writes (narrow, upstream-shaped)
- `set_default_branch_to_canonical_head`, `set_identity_head`,
  `set_remote_identity_root` — no signer (upstream takes none).
- `sign_refs(signer)` — the only signer-taking write; `SignedRefs` view.
- Typed COB writes: `Issues.open_as` / `Patches.open_as` fix the signer
  at store open (`WriteAs`); `IssueMut` (comment/edit/label/assign/
  lifecycle — label & assign REPLACE upstream sets), `Patches.create` /
  `draft`.

### Conventions
- `ValueError` only for binding validators; operational failures raise
  the `radiclepy.Error` hierarchy (`StorageError`, `RepositoryError`,
  `GitError`, `DatabaseError`, `IdentityError`, `SignerError`,
  `PolicyError`, `CobError`).
- Test-only helpers (`Profile._from_seed`, `radiclepy._test`) are
  compile-time gated behind a cargo feature and absent from release
  wheels (wheel smoke test asserts this).
