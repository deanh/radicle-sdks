# Changelog

All notable changes to `radicle-py` are documented here.

## Post-alpha Python UX (unreleased)

- Docs: the Python API reference moved from `API.md` to `docs/API.md`;
  historical plans, reviews and findings moved to `docs/archive/`, indexed by
  `docs/README.md`.
- Fix: `Repository.history(..., offset=N)` now raises `GitError` when the walk
  fails inside the skipped range (for example, a missing parent object).
  Previously `offset >= 1` returned an empty page with `has_more=False`, making
  corrupt history look like its end. Valid history is unaffected. (Issue 8f877ad)
- Docs: correct `PatchMut.review` (it returns the operation ID; upstream keeps
  an actor's first review per revision and ignores later ones), and state which
  patch lifecycle calls record an operation (`archive()` always does; a `False`
  from `ready`/`unready`/`unarchive` records nothing).
- **Minimum Rust is now 1.91** (was documented as 1.88). The pinned
  `radicle` 0.25.1 declares `rust-version = "1.88.0"` but fails to compile on
  Rust 1.88–1.90 (E0382 in `cob/identity.rs:506`); 1.91.0 is the measured
  minimum. `Cargo.toml`, README and the CI test matrix now use 1.91.
- Docs: recommend `git push rad HEAD:refs/patches` for patches from a working
  copy, and document how `Patches.create`/`draft`/`update` differ from that
  path. Fix API.md: read-only stores raise `ReadOnlyError`, not `ValueError`.
- Immutable snapshots now compare and hash structurally, recursively over all
  fields. Equal independent loads deduplicate as dict/set keys; different
  snapshots of the same entity remain distinct. Handles retain identity semantics.
  Snapshot hashes use process-local random keys (independent of PYTHONHASHSEED),
  rather than PyO3's fixed-key generated hashing of peer-controlled content.
- Accept `str | os.PathLike[str]` on filesystem inputs; bytes paths remain
  unsupported and path returns remain strings.
- Add `Profile.storage`, `rid in storage`, iteration over copied tree entries,
  issue `close`/`reopen` sugar (one lifecycle operation), and bounded
  collaboration reprs without discussion text.
- Narrow finite string domains in stubs with `Literal`; runtime validation is
  unchanged, but dynamic `str` variables may require type-checker narrowing.

- Add installed-surface stubtest checks, mypy/pyright consumer examples,
  annotation-completeness checks and a pytest-executed README quick start.
  Correct Python 3.9 identifier stubs and mark non-subclassable classes final.

### Migration from the alpha RC

| Old usage | Replacement |
|---|---|
| `init_project(s, signer, path, name, description, branch)` | `init_project(s, signer, path, name=name, description=description, default_branch=branch)` |
| `patches.create/draft(title, description, target, base, head)` | `patches.create/draft(title, description, base=base, head=head, target=target)`; target defaults to `"delegates"` |
| `patch.update(description, base, head)` | `patch.update(description, base=base, head=head)` |
| Catch `ValueError` for read-only COB writes or `StorageError` for read-only policy writes | Catch `ReadOnlyError` (inherits `Error`); unrelated validator/storage errors are unchanged |
| Identity-based value comparisons/dict keys | Snapshots compare/hash structurally; compare `.id`/`.oid` explicitly for entity identity |

Legacy positional call forms raise `TypeError`. No path-return changes,
accessor removals, or missing-object exception hierarchy were introduced.

## Alpha-RC — Git-distributed core MVP (`0.1.0`, commit-pinned)

- Review fixes: init refuses existing `rad` remotes before destructive
  upstream cleanup; `Patch.latest_revision` disambiguates summary fields from
  all-author revision slots; branch listings strip exactly one prefix.
  Regression coverage preserves remote config/tracking refs and reads persisted
  sigrefs without signing. Checkout's canonical-head/push-namespace semantics,
  raw-sort complexity and retained GIL are documented.
- Non-interactive crate adapters: `init_project`, `fork_project`,
  `checkout_project`; upstream Git executable dependency is documented.
- Typed immutable `Comment`, `Revision`, `Review` snapshots; issue discussion
  bodies and patch revision/review reads. Typed timestamps are milliseconds,
  distinct from raw operation seconds.
- Mutable/reloadable `PatchMut`, `Patches.get_mut`, revision updates,
  ready/unready/archive/unarchive, metadata/labels/assignees, revision comments,
  basic review verdicts and merge records with evaluated merge fields.
- Helper-free two-actor integration workflows, installed-artifact domain
  smoke, encrypted-key negative paths and representative stub signatures.
- Corrected lazy store-open contract and replaced the placeholder quick start.
- Alpha installation is via a Git URI, preferably a full commit SHA, not
  PyPI. Package version stays `0.1.0`; the commit identifies RC revisions.
- Deferred networking, identity governance, inline review machinery and
  generic Git mutations; no Git merge engine or automatic branch movement.

## 0.1.0 — initial implementation

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
