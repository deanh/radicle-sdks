# Plan: `radicle-py` — a comprehensive Python binding for the `radicle` crate

Status: **v3.4** — Phase 0A ✅ · Phase 1 ✅ (approved; binding scaffold, identifiers, Home/Profile/Aliases/Signer, error hierarchy, fixture strategy, feature-gated test helpers, 37 tests; CI: test job on pinned MSRV 1.88 + stable with clippy `-D warnings`, then manylinux wheel + audit + release-surface smoke). Findings: [`FINDINGS-0A.md`](./FINDINGS-0A.md). **This file (`radicle-py/PLAN.md`) is canonical**; `../radicle-py-plan.md` is a symlink to it. Links are relative to `radicle-py/`. v3.1: UTF-8 policy (bytes-first) + raw-action representation resolved. v3.2 (post-review): raw history traversal rule added to §5.5 (`entry.parents` only); COB-refs terminology corrected; packaging gate tracked in `wheels.yml`.
Reference pattern: [`../python-community-computer/`](../python-community-computer/) (PyO3 + maturin hybrid, hand-written `.pyi` stubs, API.md as the contract, pytest conformance suite, wheels CI).
Target: **published `radicle = "=0.25.1"` (crates.io) — dependency decision settled.** The crates.io 0.25.1 source is **authoritative** for API shapes; [`../heartwood/`](../heartwood/) HEAD is a useful reading reference only, since pinning unreleased HEAD would define a different target despite the unchanged package version. Exact pin (`=`, not `^`) because the binding is source-coupled and Cargo semver would otherwise let incompatible 0.25.x patch releases through; upgrade pins deliberately, per release.

### Changelog

- **v1**: initial plan.
- **v2**: revised per review — no runtime-generic COB manager; no blanket serde; writes derived from `WriteRepository`; `Node`/node-data corrections; fixtures via `Profile::init` seed; `Storage` construction corrected; stubs grow per phase.
- **v3**: revised per second review, every point re-verified against heartwood source — write methods take **no signer** (only `sign_refs` does); typed COB stores materialize `Issue`/`Patch` (not `Thread`); raw `Op` carries **multiple** action payloads (`NonEmpty<Vec<u8>>`) plus `parents`/`related`/`identity`/`manifest`; `Raw.entity` replaced by `entities`/`refs`/`history`; `RAD_KEYGEN_SEED` reading is `profile::env::seed()`, not `Profile::init`; `Aliases` is a lookup facade; `RoutingStore` opened via config-carrying `Profile::routing()`; plus small clarifications (tag peeling, `RawOp.actor`, `HeadInfo`, `branch_head`, revspec errors).

---

## 1. What this is

A new sibling project following the same engineering pattern as `python-community-computer`, but:

- **Scope**: only the `radicle` crate itself (v0.25.1).
- **Ambition**: *comprehensive within what the crate actually supports*: identity, profiles/signing, storage, read-only repository views, policy/routing/address stores, typed COB readers (issue/patch) plus raw COB inspection. Narrow, upstream-derived writes last.

### 0.25.1 facts this plan relies on (verified in source)

- `node::device` removed; signing lives at `profile::signer::Signer`.
- `logger` module removed. MSRV 1.88.
- `cob::store::Store<'a, T, Repo, Access>` is generic over a **compile-time** `T: Cob`; `Store::open_for(type_name, ...)` changes only the `TypeName`, not the materializer type. **There is no runtime-generic COB access.** `History`/ops do not serialize without a known `T::Action: Serialize`.
- `WriteRepository` provides: `set_symbolic_ref`, `set_canonical_symbolic_refs`, `set_default_branch_to_canonical_head(&self)`, `set_identity_head(&self)`, `set_remote_identity_root(&self, remote)`. **None take a signer** — only `sign_refs(&self, signer)` does. No generic branch create/delete transactions; `RefUpdate` belongs to fetch/storage internals, not this trait.
- `ReadRepository::head()` returns `(Qualified, Oid)` — the qualified ref name plus target OID.
- Typed COB stores materialize `Store<'a, issue::Issue, Repo, Access>` and `Store<'a, patch::Patch, Repo, Access>`; `Thread` is an internal field/concept inside those objects.
- `GenericOp` fields: `actions: NonEmpty<A>` (**one or more** action payloads per op), `parents: Vec<EntryId>`, `related: Vec<Oid>`, `identity: Option<Oid>`, `manifest: Manifest`. The binding's `RawOp.actions` is populated directly from `Entry::contents()` — a non-empty collection of raw action blobs — and is **not** constructed through `Op<Vec<u8>>::try_from` (fails at runtime; §5.5). A COB entity is addressed by refs → change commits → histories (`NonEmpty<Entry>`); there is no generic serialized "entity" byte value.
- `Aliases` (via `Profile::aliases()`) is a lookup facade — `alias(node_id)` / `reverse_lookup(alias)` — not an enumerable collection.
- `Home::routing(config)` requires `node::db::config::Config`; `Profile::routing()` supplies the profile's configured value. Same pattern for the database/policy stores.
- `radicle::node::Node` is a **synchronous client for the node control socket** (`struct Node { socket: PathBuf }`) — requires a running node daemon.
- Node data layer is a set of separate traits/stores: `node::policy::Store` (`follow`/`unfollow`/`seed`/`unseed`/`set_follow_policy`/`follow_policies`/`is_seeding`/…), `node::routing::Store` (`get`, `get_inventory`, `entry`, `entries`, `add_inventory`, `remove_inventory`, `count`, `prune`), `node::address::Store` (+ `StoreExt`). There is no single `Database.repositories()/track_repo()` API.
- `storage::git::Storage::open(path, git::UserInfo)` — **requires `UserInfo`**, not just a `Home`.
- `Profile::init(...)` accepts an explicit `Seed`. It does **not** read `RAD_KEYGEN_SEED` — `profile::env::seed()` does; a caller must fetch it and pass it to `init`.
- `RepositoryInfo`, `SetHead` are not `Serialize`; git2 objects are borrowed handles; a raw op's actions are opaque byte blobs, decoded per-action (§5.5).

## 2. Surface map

| Area | Upstream | Bind? | Phase |
|---|---|---|---|
| `identity` | `Did`, `RepoId`, `Doc`/`RawDoc`, `project::Project`, `Delegation`, verification | ✅ | 1 |
| `profile` | `Profile` (`load`/`init`), `Home`, `Aliases`, `profile::signer::Signer` | ✅ | 1 |
| `storage` | `Storage` (via `UserInfo`), `ReadRepository`, `RepositoryInfo`, `Remotes` | ✅ reads | 2 |
| `git` view | `Oid`, commit/tree/blob/tag contents | ✅ as **explicitly schema'd values** | 2 |
| policy store | `node::policy::Store` (follow/seed policies) | ✅ | 3 |
| routing store | `node::routing::Store` (inventory, entries) | ✅ | 3 |
| address store | `node::address::Store` | ✅ if Phase 3 spike is clean | 3 |
| `cob::store` — **typed** | `Store<issue::Issue>`, `Store<patch::Patch>` concrete adapters | ✅ | 4 |
| `cob` — **raw inspection** | COB git refs, entity enumeration, histories as `NonEmpty<Entry>`, raw ops (actions from `Entry::contents()`, §5.5) | ✅ (bytes/JSON per explicit wire spec) | 4 |
| writes | `WriteRepository::{set_default_branch_to_canonical_head, set_identity_head, set_remote_identity_root, set_symbolic_ref}` (no signer) + `sign_refs(signer)` + typed COB actions | ✅ narrow set only | 5 |
| `node::Node` control socket | requires running daemon; out of local-data scope | ❌ v1 | — |
| p2p transport/`sync`, `web`, `cli`, `test`, `logger`-adjacent | — | ❌ | — |

**Non-goals (v1)**: p2p networking; `node::Node` (excluded because it needs a running node daemon — *not* because of async: it's a sync socket client); runtime-generic COB **materialization as domain objects** (compile-time `T: Cob`; upstream has no runtime materializer — loading raw history as `NonEmpty<Entry>` is feasible and is what `Raw` does); binding-owned generic git branch mutation (only if demanded later, explicitly documented as not-from-upstream).

## 3. Proposed Python API surface

Notation: `→` return type; attributes without parens, methods with parens (this distinction is binding law for API.md and stubs).

```python
import radiclepy as rad

# ── Identifiers: str at the boundary, aliases in stubs ───────────
# stubs: RepoId = str; Did = str; NodeId = str; Oid = str; ObjectId = str
rad.parse_repo_id / parse_did / parse_node_id / parse_oid   # validate + canonicalize; ValueError on bad input

# ── Profile & keys ───────────────────────────────────────────────
rad.Profile.load() -> Profile
profile.did -> str          # attributes: did, node_id, home
profile.home -> rad.Home    # path resolution (RAD_HOME / ~/.radicle)
profile.signer() -> Signer  # keystore, RAD_PASSPHRASE, ssh-agent (upstream behavior)
profile.aliases -> rad.Aliases         # lookup facade, not a collection
aliases.get(node_id) -> str | None     # upstream alias()
aliases.reverse_lookup(alias) -> dict[str, list[str]]

# ── Storage & reads ──────────────────────────────────────────────
store = rad.Storage.from_profile(profile)   # primary ctor (supplies UserInfo from profile);
                                            # low-level: Storage.open(path, alias=..., node_id=...)
repo  = store.repository(rid) -> Repository # RepositoryError if absent
repo.rid -> str
repo.head -> HeadInfo                       # attributes: rid, head; upstream head() is (Qualified, Oid)
repo.description() -> str | None            # methods: description(), identity(), …
repo.identity() -> ProjectDoc               # parsed project doc + verify()
repo.branches() -> list[str]
repo.branch_head(name) -> Oid | None        # OID a branch points at; commits via commit(oid)
repo.tags() -> list[TagInfo]                # raw target + fully-peeled target and its object kind
repo.rev_parse(rev) -> Oid                  # GitError on invalid OR unresolved revspecs — see errors
repo.commit(oid) -> Commit                  # schema'd value, see §5.4
repo.tree(oid) -> Tree                       # entries ordered, modes as documented ints
repo.blob(oid) -> bytes
repo.history(rev, *, limit, from_=None) -> HistoryPage  # page-based, eager; no lazy iterators
repo.remotes() -> list[RemoteInfo]

# ── Policy & routing (named after upstream stores) ───────────────
policy = profile.policy_store()             # primary ctor (profile supplies paths + db config)
policy.follow(node_id, alias=None) / unfollow(node_id) -> bool
policy.seed(rid, scope) / unseed(rid) -> bool
policy.follow_policy(node_id) / seed_policy(rid) -> dict | None   # schema'd
policy.is_following(node_id) / is_seeding(rid) -> bool
policy.follow_policies() -> list[FollowPolicyEntry]

routing = profile.routing_store()           # primary ctor: Home::routing needs node::db::config::Config;
                                            # Profile::routing() supplies the configured value.
                                            # Any low-level ctor must take db path AND config.
routing.get(rid) -> list[NodeId]
routing.get_inventory(node_id) -> list[RepoId]
routing.entries() -> list[RoutingEntry]        # eager list; count documented
routing.count(rid) -> int

# ── COB: typed readers (Phase 4) ─────────────────────────────────
issue = rad.issue.Issues.open(repo)            # wraps Store<'_, issue::Issue, _>
for iid, issue in issue.all(): ...             # eager, bounded by repo size; page variant ok
issue.get(iid) -> Issue | None

patch = rad.patch.Patches.open(repo)           # same shape for patch::Patch

# ── COB: raw inspection (Phase 4) ────────────────────────────────
raw = rad.cob.Raw.open(repo)
raw.typenames() -> list[str]                     # from COB git refs
raw.entities(typename) -> list[ObjectId]         # entity ids from COB refs
raw.refs(typename, entity_id) -> list[CobRef]    # COB refs pointing at change commits
                                               # (NOT "signed refs": authenticity comes from
                                               #  rad/sigrefs; COB refs aren't individually signed)
raw.history(typename, entity_id) -> list[RawOp]  # runtime-generic: load entries and evaluate to
                                                 # NonEmpty<Entry> — no domain type required
class RawOp:                                     # full wire spec in API.md;
    id: str                                      #   one op = NON-EMPTY list of actions
    actor: str                                   # canonical NodeId (upstream stores a PublicKey;
                                                 #   exactly one representation — see §5.5)
    timestamp: int
    actions: tuple[bytes, ...]                   # each action decoded independently
    parents: tuple[str, ...]
    related: tuple[str, ...]
    identity: str | None
    manifest: CobManifest
    def actions_json(self) -> tuple[object, ...]  # raises CobError if any action isn't valid canonical JSON

# ── Writes (Phase 5, narrow, each maps 1:1 upstream) ─────────────
# These take NO signer upstream — the Python wrappers must not take one either.
# No hidden sign_refs step: composing write-then-sign is the caller's explicit sequence.
repo.set_default_branch_to_canonical_head() -> SetHeadInfo  # → set_default_branch_to_canonical_head
repo.set_identity_head() -> Oid                             # → set_identity_head
repo.set_remote_identity_root(remote) -> Oid                # → set_remote_identity_root
repo.sign_refs(signer) -> SignedRefs                        # the only signer-taking write here
                                                            # (upstream returns SignedRefs directly;
                                                            #  SignedRefsInfo name is reserved for RepositoryInfo.refs)
# + typed COB action APIs (issue/patch) once upstream action types are confirmed signable

# ── Errors ───────────────────────────────────────────────────────
# Convention: binding-validator failures (parse_*) → ValueError; operational failures → hierarchy.
# Revspecs: rev_parse raises GitError for BOTH syntactically invalid and unresolved-but-valid
# revisions — revspec grammar is git's, not ours. ValueError is reserved for our own parse_* validators.
radiclepy.Error
├── StorageError          # storage::Error (incl. is_not_found mapping)
├── RepositoryError       # storage::RepositoryError
├── GitError              # raw git2 failures crossing the boundary
├── DatabaseError         # node::db / store open failures
├── IdentityError
├── SignerError
├── PolicyError
└── CobError
```

## 4. Project layout

```
radicle-py/
  Cargo.toml            # pyo3 0.23 (abi3-py39), pythonize 0.23, radicle = "=0.25.1"
  pyproject.toml        # maturin, python-source = "python", stub includes
  README.md
  API.md                # contract-first reference incl. conversion schemas (§5)
  .github/workflows/wheels.yml   # ubuntu + macOS matrix (rust 1.88); see §6 for manylinux/Windows notes
  src/
    lib.rs
    error.rs            # hierarchy + mappers per upstream error enum
    identity.rs         # Did/RepoId/Project + parse_*
    profile.rs          # Profile, Home, Aliases, Signer
    storage/
      mod.rs            # Storage (from_profile / open+UserInfo equivalent)
      repository.rs     # reads
      write.rs          # Phase 5 only; one method per upstream call
    values.rs           # Commit/Tree/TagInfo/etc — hand-written conversions, §5.4
    policy.rs           # PolicyStore
    routing.rs          # RoutingStore
    addresses.rs        # address store (Phase 3, if clean)
    cob/
      mod.rs
      raw.rs            # raw ref/history/op inspection (wire spec §5.5)
      issue.rs          # Store<issue::Issue> adapter
      patch.rs          # Store<patch::Patch> adapter
  python/radiclepy/
    __init__.py  __init__.pyi  cob.pyi  issue.pyi  patch.pyi  py.typed
    _types.pyi          # RepoId = str, Oid = str, … aliases shared across stubs
  tests/
    conftest.py         # fixture strategy §5.6
    test_smoke.py  test_identity.py  test_profile.py
    test_storage.py  test_repository.py  test_values.py
    test_policy.py  test_routing.py
    test_cob_raw.py  test_issue.py  test_patch.py
    test_api.py         # API.md conformance: documented names exist with documented kinds
```

## 5. Design decisions

1. **Identifiers are strings** at the boundary (same as sibling project). Stubs define shared aliases (`RepoId = str`, `Oid = str`, …) in `_types.pyi` so typed callers get documentation without a runtime class hierarchy. Revisit as pyclass value types only if demand appears.
2. **No runtime-generic COB manager.** Two explicit layers: **raw COB inspection** (enumerate typenames/entities from COB git refs, load histories as `NonEmpty<Entry>`, expose raw `Op`s per wire spec) and **typed adapters** (`Store<issue::Issue>`, `Store<patch::Patch>`). Raw access gives refs/entities/histories for any typename; per-typename *materialization as domain objects* is out (no upstream runtime materializer).
3. **No blanket serde/pythonize.** `pythonize` is used only where the upstream type is genuinely `Serialize` with a stable shape (e.g. `Project` doc). Everything else gets a **hand-written conversion with a schema documented in API.md**.
4. **Value conversion specs (API.md sections, each mandatory before its phase ships):**
   - `Commit`: author/committer name/email encoding; commit vs author timestamp semantics (TZ handling); message bytes vs str (invalid UTF-8 → `bytes` field or documented replacement policy).
   - `Tree`: entry ordering (git sort order stated explicitly), mode encoding (int, with constants), submodule/symlink entries.
   - `TagInfo`: raw tag target OID vs **fully-peeled** target ("peeled" = resolve the tag chain to the final object — tags may target commits, trees, blobs, or other tags; NOT specifically "the commit"); expose the peeled object's `kind`; annotated vs lightweight distinguished.
   - Invalid UTF-8 in any name/message field: **resolved by Phase 0A evidence** — bytes-first fields (`name: bytes`, `message: bytes`; all raw-byte accessors exist in git2 and a non-UTF-8 path survives write+read-back) plus documented lossy conveniences (`name_str`, `message_str`).
   - Signatures/OIDs on the wire: lowercase hex strings, always.
   - Git/COB objects are returned as **immutable pyclass instances with read-only properties** (not bare dicts), so types evolve without breaking typing. History/branch listings are `list`s of these.
5. **Raw COB op wire representation (API.md)** — corrected by Phase 0A: action blobs come from `entry.contents().iter()` (one raw JSON blob per action, non-empty by construction), **not** from `Op<Vec<u8>>::try_from(&entry)` (panics/fails at runtime: serde can't deserialize a JSON object into `Vec<u8>`). Each action is decoded independently by `actions_json()`. `id` hex string. `actor` has exactly one representation — **canonical NodeId string** (upstream stores a `PublicKey`; NodeId chosen because that is the native identity of a COB actor; DID derivation is the caller's job). `timestamp` integer (epoch ms — match upstream `Timestamp`). `parents`, `related`, `identity`, `manifest` exposed per the class sketch in §3. `actions_json()` raises `CobError` if any action isn't valid canonical JSON/UTF-8. Known typed actions (issue/patch) are consumed via the typed adapters, not via `actions_json()`.
   - **Traversal rule (0A review):** a COB change commit has three categories of git parents —
     causal tips, related objects, and the identity/resource commit. `Raw.history` walks
     **`entry.parents` only**; `entry.related` and `entry.resource()` (identity) are exposed as
     links but never traversed. Walking raw git `parent_ids()` wrongly pulls the identity COB's
     entries into the domain COB's history (a real bug in spike rev 1). A fresh issue is one op
     with two actions; its identity commit belongs to the identity COB and is *not* in the
     issue's history.
   - **Phase 4 implementation notes (from 0A review):** `Raw.history` must deduplicate entries
     **globally across all COB refs of the entity**, not per ref — multiple namespaces routinely
     carry overlapping histories (the spike's per-ref `visited` is deliberately simplified).
     Result **ordering must be explicit**: causal topological order (documented in API.md), not
     ref-enumeration order.
6. **Owned handles, reopen-per-op** (as sibling project). Consequence for iteration: **no lazy iterators holding storage/git handles open**. `history()` is page-based (`limit`, `from_`, returns eager `HistoryPage`); store reads (`issue.all()`) return eager lists. Bounded, predictable handle lifetimes.
7. **Test fixtures — no new escape hatch.** `Profile::init` takes an explicit `Seed` and does **not** read `RAD_KEYGEN_SEED` itself (`profile::env::seed()` does — the caller must fetch and pass it). Fixtures: `Profile::init(temp_home, alias, None, seed)` with a deterministic seed; unencrypted temp keys avoid ssh-agent and `RAD_PASSPHRASE` entirely. Fixtures pass explicit paths/`Home` everywhere — **no process-global `RAD_HOME` mutation** (parallel-pytest safe). **Public Python init contract:** `Profile.init(...)` generates a secure random seed — implementation note: `Seed::new([u8; 32])` exists but there is **no production `generate()`**, so we fill 32 bytes from the OS RNG (likely `getrandom`, matching radicle-cli's key generation); deterministic seed injection stays private/test-only — not shipped as a documented Python parameter, exercised from the Rust side of the binding's fixtures only.
8. **Errors**: `ValueError` for our own validator failures (`parse_*` only); domain hierarchy for operational failures — including revspecs, where `GitError` covers both invalid and unresolved (`rev_parse` semantics are git's, not ours). Every mapper function documents which upstream error enum it covers; `GitError`/`DatabaseError` added for low-level crossings.
9. **Writes are enumerated, not "transactional," and signer parameters follow upstream exactly:** none of the `set_*` methods take a signer; only `sign_refs(signer)` does. A wrapper that silently wrote-then-signed would break the 1:1 mapping and introduce partial-failure semantics — not allowed without its own explicitly documented failure contract. Anything not upstream-shaped (generic branch create/delete) stays out; if later needed, it ships as explicitly *binding-owned* over raw git2 with that stated in API.md.
10. **API.md is the contract from day one**; `test_api.py` verifies documented names *and* method-vs-attribute kinds. **Stubs ship per phase, not in a final sweep** — each phase ends with its `.pyi` + API.md sections + tests complete.

## 6. Packaging & CI notes

- **Linux wheels**: build under manylinux (maturin default). Must document build prerequisites: the bundled SQLite and libgit2 build from source — no system packages required in the final wheel, but the *build* image needs a C toolchain; confirm in Phase 0A spike.
- **macOS**: arm64 + x86_64 as sibling project does.
- **Windows**: upstream supports it, but v1 ships **Linux + macOS** (matching the sibling matrix) with Windows explicitly listed as "unsupported, unverified" in README. Revisit post-v1.
- abi3-py39, Python ≥ 3.9, maturin hybrid, `python-source = "python"` — identical to sibling.

## 7. Sequenced phases (revised)

| Phase | Deliverable | Gate / risk |
|---|---|---|
| **0A — Feasibility spikes** ✅ complete, ◐ packaging gate → first `wheels.yml` run ([findings](./FINDINGS-0A.md)) (throwaway code, in `spike/` + `wheel-probe/`) | Build against `=0.25.1` (crates.io source authoritative). Deterministic temp profile: seed constructed explicitly, passed to `Profile::init`. Open a repo, hand-convert one commit + one tree. Load raw COB history for an arbitrary typename as `NonEmpty<Entry>`; validate `entities`/`refs` enumeration against refs. Confirm manylinux wheel build with bundled sqlite/libgit2. Check PyPI `radicle-py`. | Decides §5.4 UTF-8 policy, COB ref layout assumptions, dependency pin, spike-derived API corrections. Exit = updated plan, then implement. |
| **1** ✅ | Identifiers + aliases in stubs, `Home`, `Profile` (`load`/`init` surface), `Signer`, error hierarchy, fixture strategy from §5.7 committed as `conftest.py` | Done. Notes: deterministic seeding shipped as private `Profile._from_seed` (test-only, per §5.7); `Aliases.get` on own node id returns the profile alias (upstream seeds it) — documented as "known alias or None" |
| **2** | `Storage.from_profile`, read-only `Repository`, value classes with API.md schemas (commit/tree/tag/blob/history pages/remotes) | Highest user value; handle-lifetime rules from §5.6 |
| **3** | `PolicyStore`, `RoutingStore` named after upstream traits, config-carrying primary ctors via `profile.*_store()`; address store if spike-clean | Verify DB open/lock semantics |
| **4** | Typed `issue`/`patch` readers + raw COB inspection with wire spec | Second-highest risk: op/serialization fidelity — validate against fixtures produced by real upstream writers |
| **5** | Narrow writes: `set_default_branch_to_canonical_head`, `set_identity_head`, `set_remote_identity_root`, `sign_refs`, typed COB action writes *only where signing/atomicity behaviour is confirmed* | Deferred by design (review): read-focused first release |
| **6** | Wheels CI polish (manylinux matrix), README install matrix, version 0.1.0 | Low risk |

(Stub completion is not a phase — it's part of every phase, §5.10.)

## 8. Resolved open questions (from v1) & remaining

**Resolved by review:**
- Names: `radiclepy` import / `radicle-py` distribution ✅ (pending PyPI availability check — do in Phase 0A).
- Issues/patches: in, as typed readers (not generic materializer).
- Git objects: immutable classes with documented schemas, not "whatever serde emits."
- Test keys: existing `Profile::init` seed path; no new dev hatch.
- Writes: read-focused first release; Phase 5 narrow set.

**Resolved by second review (each verified in heartwood source):**
- `Aliases` shape: lookup facade (`get`/`reverse_lookup`), not enumerable — fixed in §3.
- Write signatures: no signer except `sign_refs` — fixed in §3 and §5.9.
- Routing store construction: config-carrying `profile.routing_store()` primary — fixed in §3.
- `RawOp`: multiple actions + full field set; `Raw.entity` removed — fixed in §3/§5.5.
- Tag peeling semantics, `RawOp.actor`, `HeadInfo`, `branch_head`, revspec error class — fixed in §3/§5.4.

**Resolved since v3:** dependency pin — `radicle = "=0.25.1"` from crates.io; the published crates.io source is authoritative (heartwood HEAD is ahead of the release with an unchanged package version, so it can't be the target).

**Resolved during Phase 0A:**
1. Invalid-UTF-8 policy — **bytes-first + lossy `*_str` conveniences** (see §5.4; evidence in FINDINGS-0A §S3).
2. PyPI — `radicle-py` and `radiclepy` both available (404, checked 2026-07-28).

**Still open:**
1. Whether the address store is in Phase 3 scope or parked.
2. CI task: manylinux rebuild + auditwheel check (host wheel tagged manylinux_2_34; local docker unusable). `wheels.yml` added (maturin default glibc manylinux container — explicitly *no* musl tooling) to close this gate on its first run.

---

*Sources: [`../python-community-computer/`](../python-community-computer/); [`../heartwood/`](../heartwood/) monorepo, `crates/radicle` — `storage.rs` / `storage/git.rs` (`WriteRepository` without signer; `sign_refs(signer)`; `head() -> (Qualified, Oid)`; `Storage::open(path, UserInfo)`), `cob/store.rs` (`Store<'a, T, _>`, `open_for`), `cob/issue.rs` / `cob/patch.rs` (`Issue` / `Patch` materializers, `Thread` internal), `cob/op.rs` (`GenericOp`: `actions: NonEmpty<A>`, `parents`, `related`, `identity`, `manifest`), `profile.rs` (`Profile::init(seed)`, `env::seed()` / `RAD_KEYGEN_SEED`, `Aliases` lookup facade, `routing(config)`), `node.rs` (`Node` control socket), `node/policy/store.rs`, `node/routing.rs`, `node/address/store.rs`. Cross-checked against the crates.io `radicle` 0.25.1 tarball. Both review rounds verified in source before incorporation.*
