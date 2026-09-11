# FINDINGS-05 — Phase 5 (narrow upstream-shaped writes)

Evidence baseline: `radicle = "=0.25.1"` (crates.io), verified against
`~/.cargo/registry/src/.../radicle-0.25.1`.

## Repository writes are trait methods on the handle we already hold

`set_default_branch_to_canonical_head` / `set_identity_head` /
`set_remote_identity_root` live on `storage::WriteRepository`
(`src/storage.rs`), `sign_refs` on `storage::SignRepository`; both traits
are implemented by the very `storage::git::Repository` our `PyRepository::open()`
already returns — no separate "writable handle" type exists. The binding
exposes them 1:1, with the four-method surface of PLAN §3 and nothing more.

- `SetHead { old: Option<Oid>, new: Oid }` + `is_updated()` — shipped under
  the upstream name `SetHead` (the §3 sketch's `SetHeadInfo` lost to
  upstream naming, same rule that kept `SignedRefs`).
- `set_remote_identity_root` is **unconditional upstream**: it computes the
  repo's canonical identity root and writes the remote's ref with no
  membership check — an unknown remote simply gets a ref created; idempotent
  on repeat (test-proven). `set_identity_head`/`set_default_branch_to_canonical_head`
  are quorum/delegate-derived and no-op-stable on a fresh fixture repo.
- `sign_refs`: upstream `SignedRefs` exposes `id()`, `at`, `parent()`,
  `feature_level()`, `refs()` (BTreeMap of refname→oid). The signature bytes
  field is **private with no getter** — deliberately not worked around: the
  signature is verified upstream on every read path. Signing an unchanged
  repo reuses the existing sigrefs commit (same `at`) — test-proven.

## COB write shape: signer lives in the access mode, never in the method

Upstream mutation impls are keyed on `Issues<'_, Repo, WriteAs<'_, Signer>>`
/ `Patches<…>`: you open the store with `WriteAs::new(&signer)` and the
mutators (`comment`/`edit`/`label`/`lifecycle`/…) take no signer. The binding
mirrors this: `Issues.open_as(repository, signer)` / `Patches.open_as(...)`,
then signer-less mutators. Each mutator is one COB transaction (upstream
`transaction()` commits immediately); `NoCache` is the only cache (no node
runtime to own one).

- `Action::Label` **replaces** the label set (`self.labels =
  BTreeSet::from_iter(labels)`; upstream `test_issue_label` re-labels with
  full sets). Shipped verbatim + loudly documented. There is no `unlabel`.
- `IssueMut::comment` takes a **mandatory** `CommentId` reply target
  upstream; the binding defaults it to the root comment (documented).
  `edit_description` = root-comment edit (upstream).
- `CommentId = change::EntryId = Oid` — same 40-hex type as git Oids; no
  conversion.
- `Title`: non-empty, no `\n`/`\r`, trimmed — **no length cap**.
- `CloseReason::{Other => "unspecified", Solved => "solved"}` (Display) —
  the lifecycle `reason` accepts exactly those two strings.
- `patch::TargetBranch` is constructor-qualified (`src/cob/patch.rs`): bare
  `master` is stored as `refs/heads/master`; non-`refs/heads/*` qualified
  refs are rejected. `Patch.target` strips the guaranteed prefix so input
  `branch:release` ⇢ output `branch:release`.
- `Patches::create/draft(title, description, MergeTarget, base, head,
  &labels, cache)` — `base`/`head` are git Oids (`impl Into<git::Oid>`).
  The broader `PatchMut` surface (revisions, review, merge, close) exists
  upstream but is NOT wrapped: PLAN §3 pins Phase 5 to create/draft.

## Binding-structural facts (cost real debugging time)

- `profile::Signer` is **not `Clone`** (SSH-agent variant wraps
  `RefCell<Agent>` ⇒ also not `Sync`). `PySigner` shares it via `Rc`
  (`Arc` trips clippy `non_send_fields_in_send_ty`-adjacent lints; pyclass
  is `unsendable` anyway) and derives `Clone` cheaply. **Any pyclass
  holding a `PySigner` must be `unsendable`** (`Issues`, `IssueMut`,
  `Patches`) or the borrow-check-equivalent pyo3 thread assertion fires.
- pyo3 0.23 allows **one `#[pymethods]` block per class**: Phase 5 repo
  writes are thin wrappers in `storage.rs`'s block delegating to
  `write.rs` (PLAN's `storage/write.rs` intent, flat-module form).
- macro_rules hygiene: a mutator helper macro must receive the binding
  ident (`issue_tx!(self, |m| { … })`) — an `m` invented inside the macro
  definition is invisible to the call-site body.

## Error mapping

All four repo writes → `RepositoryError` (upstream `storage::RepositoryError`).
Read-only store used for writes → `ValueError` (our validator class:
programming-model misuse, not an operational failure). Bad titles/labels/
DIDs/targets/OIDs → `ValueError`; missing entities → `CobError`.
