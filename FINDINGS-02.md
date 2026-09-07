# Phase 2 — Storage & read-only repository: spike findings

Date: 2026-07-28 · Spikes: [`spike/src/bin/phase2.rs`](./spike/src/bin/phase2.rs) (throwaway) · Plan: [`PLAN.md`](./PLAN.md) §7 Phase 2

Answers the schema questions Phase 0A deferred (tree ordering, tag peeling) and
settles the read-only surface against the published `radicle = "=0.25.1"`.

## S5 — Tree ordering (the 0A-deferred question) — RESOLVED

- libgit2's `Tree::iter()` yields entries in **canonical git tree order**:
  names sorted as if directory entries carried a trailing `/`.
- Proven different from a naive byte-sort of raw names: with a directory `foo`
  and a file `foo!`, git compares `"foo/"` vs `"foo!"` and `'!'` (0x21) < `'/'`
  (0x2f), so git yields `["foo!", "foo"]`; a naive byte-sort of the raw names
  puts the shorter prefix `"foo"` first → `["foo", "foo!"]` (observed, differ).
- **Binding contract:** `Tree.entries` preserves libgit2 walk order verbatim.
  The binding does NOT re-sort; canonical order is exactly what libgit2 returns.
  `TreeEntry` exposes `name: bytes`, `kind` ("blob"|"tree"|"link"|"commit"|None),
  `mode: int` (octerms like 0o100644), `oid: Oid`.

## S6 — Tags: raw vs peeled vs kind — RESOLVED (with a trap)

- A tag ref target object has `kind`:
  - `Some(Tag)` → **annotated** tag (a tag object; has a message/tagger).
  - `Some(Commit)`/`Some(Blob)`/`Some(Tree)` → **lightweight** tag (ref points
    directly at the target).
- **Peel trap:** `Object::peel(ObjectType::Any)` is only meaningful when the
  object is itself a tag. On a *lightweight* tag (raw kind already `Commit`),
  `peel` returns the commit's **tree** — silently wrong. Observed:
  lightweight `plain` → `peeled_kind=Some(Tree)`.
- **Binding contract (`TagInfo`):**
  - `name: str`, `target: Oid` (the ref target, always present),
  - `kind: "annotated" | "lightweight"` (from raw object kind == Tag or not),
  - `peeled: Oid | None` and `peeled_kind: str | None` — **present only when
    kind == "annotated"** (peel the tag object); for lightweight tags the target
    IS the peeled object, so peeled == target and we still report the concrete
    `peeled_kind` (commit/tree/blob/tag). "Peeled" means fully resolved through
    any tag chain ( ObjectType::Any ), NOT specifically to a commit.

## S7 — History pages — RESOLVED (constraint noted)

- `ReadRepository::revwalk(head)` returns libgit2's `Revwalk`, which implements
  `Iterator<Item = Result<Oid>>`. Pagination via `Iterator::{skip, take}` works.
- **Constraint:** `git::raw::Sort` is `pub(crate)` in the published crate — the
  binding cannot set TOPO/TIME sorting. Order is libgit2's default (insertion /
  roughly reverse-chronological for linear history). For merge-heavy history
  this is not a guaranteed topological order.
- **Binding contract (`Repository.history`):** returns `(ref_name, [Oid])` for a
  bounded page `history(rev, *, limit, offset)`; ordering documented as
  "libgit2 default revwalk order (not re-sorted; explicit sorting is not
  exposed by the pinned upstream)". A future upstream that re-exports `Sort`
  can tighten this. `limit` is required (bounded); default cap enforced binding-side.

## S8 — Remotes — RESOLVED

- `RemoteId = PublicKey` (type alias). `Repository::remotes()` is the *inherent*
  method returning `Iterator<Item = Result<(PublicKey, Remote)>>` (NOT the trait's
  `Remotes` container); each `Remote.refs` is a `SignedRefs` (use `.count()` on
  its `refs`, which is a `Refs`).
- `references_of(remote)` → `Refs` with `.len()` and iteration `(RefStr, Oid)`.
- **Binding contract (`RemoteInfo`):** `fetcher: NodeId`, `refs_head: list[str]`
  (names from the remote's signed refs), plus `references_of()` detail. After
  `rad::init`, a repo has exactly one remote (itself) with the canonical refs:
  `refs/rad/id`, `refs/rad/root`, `refs/rad/sigrefs`, `refs/heads/<branch>`,
  and COB refs under `refs/cobs/…`.

## S9 — Identity document — RESOLVED

- `ReadRepository::canonical_identity_doc()` → `DocAt` (Deref to `Doc`):
  `.project()` → `Project { name(), description(), default_branch() }`,
  `.commit`, `.blob`, `.version().number()`, `.delegates()`.
- `ReadRepository::default_branch()` → `Qualified` (e.g. `refs/heads/master`).
- **Binding contract (`ProjectDoc`):** `name`, `description`, `default_branch`
  (the short branch name from the project payload), `version: int`,
  `delegates: list[Did]`, `threshold: int`, `visibility: str`, and the raw
  `commit`/`blob` OIDs. Verification (`Doc::verify`) is exposed as a method.

## Storage / Repository construction

- `Storage` obtained via `profile.storage` (`radicle::Storage`); a binding-side
  `Storage.from_profile(profile)` and low-level `Storage.open(path, alias,
  node_id)` (needs git::UserInfo) construct it. `storage.repository(rid)` opens
  read-only. `Repository` holds `Arc<Profile>` + `rid` and reopens per operation
  (PLAN §5.3), so it is `Send`-safe and never holds a live git handle.
- `repo.head` → `HeadInfo(name, oid)` from `ReadRepository::head()` (confirmed
  `(Qualified, Oid)` in 0A).

## Plan deltas for §5.4 / §3 (Phase 2)

1. §5.4 Tree: ordering contract = libgit2 walk order (canonical), never re-sorted.
2. §5.4 Tags: peel only when kind == Tag; lightweight tags carry target==peeled
   with the concrete kind; "peeled" = fully resolved, not commit-specific.
3. §3/§5: `Repository.history` ordering caveat (no upstream `Sort`); limit required.
4. New §: `Storage` construction; `RemoteInfo` shape; `ProjectDoc` shape.
