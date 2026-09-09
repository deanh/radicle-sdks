# Phase 4 — Typed + raw COB readers: findings

Date: 2026-07-28 · Plan: [`PLAN.md`](./PLAN.md) §7 Phase 4

Verified against crates.io `radicle = "=0.25.1"` + `radicle-cob = 0.21.1`.

## Typed readers

- Read-only typed stores: `Store::open(repo, access::ReadOnly)` works for
  both `cob::issue::Issues` and `cob::patch::Patches` (they `Deref` to
  `Store`, so `get`/`all`/`count`/`is_empty` come from there). `all()` yields
  `(ObjectId, T)` — materialize eagerly into Python value objects; the
  binding stores no borrowed git handles (PLAN §5.6).
- **Upstream types are `String`-typed** where it matters (issue title,
  comment bodies, labels, patch titles/descriptions): already valid UTF-8 by
  construction, so these fields are `str` in the binding — the bytes-first
  policy is for byte-typed sources (git messages/paths), not UTF-8-guaranteed
  COB payloads. Documented in API.md.
- `Issue::description()` = **root comment body**; `Patch` description =
  **latest revision's description** (`patch.latest()`; Revision fields are
  private — use `base()`/`head()` accessors, not fields).
- `patch.version()` returns `RevisionIx` = latest **revision index** (0-based),
  not a "document version".
- Typenames: `xyz.radicle.issue`, `xyz.radicle.patch` (and `xyz.radicle.id`
  for the identity COB).

## Raw inspection

- All store plumbing lives in `radicle-cob` traits implemented by the storage
  `Repository`: `cob::object::Storage` (`types`/`objects`) +
  `cob::change::store::Storage` (`load`). ObjectId/Resource types are
  `radicle::git::Oid` throughout (the radicle-oid newtype *is* git::Oid here)
  — no conversions needed (a first-draft `Oid::from_raw` was spurious).
- COB ref names are the **full namespaced form**:
  `refs/namespaces/<nid>/refs/cobs/<typename>/<entity>/<rest>` (exposed
  verbatim as `CobRef.name`).
- Raw actions from `entry.contents().iter()` (one raw JSON blob per action) —
  `Op<Vec<u8>>::try_from` fails at runtime (confirmed in 0A; still true).
- `Entry.timestamp` (radicle-cob) is a plain `u64` **epoch seconds**
  (`radicle-cob change::store::Timestamp` — "Local time in seconds since
  epoch"; the node layer converts with `Timestamp::from_secs`). Do NOT
  confuse with `node::Timestamp` (millis) — Phase 3's routing timestamps
  really are millis. The binding surfaces the COB value verbatim (seconds)
  and says so on the field. Regression: timestamp-unit test asserts the
  value sits in a wall-clock **seconds** window. `RawOp.actor` =
  `entry.author()` — a `crypto::PublicKey` (canonical NodeId), matching
  PLAN §3.
- `RawOp.identity` = `entry.resource()` — the identity-COB **link**; exposed
  but never traversed (same rule as `related`). A fresh issue's first op
  carries **3** actions when created with labels (comment + edit + label);
  2 without.
- `change::store::Version` has no `Display` — `usize::from(version)`.

## History walk contract (implemented + tested)

- Traverse `entry.parents` only; **global visited set across all refs of the
  entity** (multi-namespace histories overlap).
- Output ordering: Kahn topological sort (each op emitted only after all its
  loaded parents), lexicographic tie-break by op id. Must return an
  **insertion-ordered Vec** — an early draft collected into a `BTreeMap` and
  silently re-sorted lexicographically (test with 4-op chain catches this).
- Cycles are a **corruption error**, not a degraded ordering: content-
  addressed causal history is acyclic by construction, so if Kahn's
  algorithm cannot emit every op the binding raises `CobError`
  ("corrupt COB history: causal cycle…") instead of silently returning a
  non-topological tail (an earlier draft appended the remainder — dropped
  after review: a broken ordering contract is worse than a loud failure).
- The identity COB entry is reachable via the issue op's `identity` link but
  **not** in its history (regression test `test_history_does_not_traverse_identity`).

## COB ref acceptance is strict (Phase 4 review)

- `cob::object::parse_refstr` accepts **any** `refs/<category>/<typename>/<oid>/…`
  shape — it checks the *shape*, not that the category is `cobs`. A tag ref
  like `refs/tags/xyz.fake.type/<oid>` false-matches (test-proven).
- `Raw.typenames()`/`Raw.entities()` therefore parse via
  `cob::TypedId::from_namespaced`/`from_qualified`, which require
  `refs/cobs/<typename>/<id>` after namespace stripping. Non-COB refs of
  COB shape never appear (regression: `_test.create_ref` plants fake
  tag-/branch-shaped lookalikes). Note upstream `repo.types()` still uses
  the loose parse; the binding's `entities()` does not call it.

## Contract drift round-trip (Phase 4 review)

The first RawOp cut drifted from the PLAN §3 wire sketch (`list` vs
`tuple`, loose `type_name`/`version` fields vs `manifest`,
`action_json_types()` probe vs `actions_json()`). The PLAN contract won:
`actions`/`parents`/`related` are **tuples**, `manifest: CobManifest`
(typename + version), and `actions_json() -> tuple[object, ...]` decodes
each action to plain Python objects (`CobError` on invalid JSON). The
half-measure `action_json_types()` is gone.

## Phase 5 preview (verified available, NOT exposed)

- `IssueMut::comment/edit/label/lifecycle/...` and
  `Patches::create/draft(...)` with `WriteAs` signer are exercised by the
  test-only helpers (`_test.create_issue`/`create_patch`, comment chains).
  Phase 5 wraps them publicly; `NoCache` is a valid `cache::Update<T>`.
