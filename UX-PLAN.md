# Pythonic UX plan (post-alpha)

Status: **implementation on `pythonic-ux`, revised after UX-PLAN-REVIEW.md**.

Execution record:
- Patch 1 implemented: PathLike inputs (including custom fspath and rejected
  byte paths), Literal contracts, reprs, profile storage, lifecycle sugar and
  cheap protocols. Helper build: 186 tests passed.
- Patch 2 implemented: all 21 snapshot classes have structural equality/hash;
  tests cover independent loads, nested-only differences, same-ID changes,
  unrelated types, copied collections and identity handles.
- Patch 5 implemented: keyword-only metadata/revision bounds and uniform
  ReadOnlyError, with updated callers and migration notes. Helper build:
  190 tests passed.
- Supporting Patch 6 tooling implemented: stubtest with two narrow documented
  exceptions, mypy/pyright consumer checks, 100% annotation completeness and a
  pytest-executed README quick start. CI checks helper and installed-wheel
  surfaces, including oldest-Python typing.
- Final validation: 192 helper-build tests; clean release-wheel installations on
  Python 3.9 and 3.14 each pass 79 tests (113 legacy-fixture skips), release
  smokes, stubtest, consumer typing and annotation completeness. Clippy passes
  in both feature states. Local wheel is manylinux_2_39 x86_64, not a substitute
  for native manylinux_2_28/macOS CI; Rust 1.88 and live SSH remain unverified.
- Patches 3 and 4 remain consumer-triggered; no datetime views or store-close
  protocol were added. No Radicle submission/merge has been performed.

Original scope follows. Scope: remove concrete
friction from the *existing* alpha surface. "Pythonic" is not a completeness
target: each item must fix an observed problem. No new upstream functionality,
no change to the upstream pin.

## Guardrails (unchanged from PLAN.md / ALPHA-PLAN.md)

- Identifiers stay `str` at runtime (§5.1). No rich identifier classes.
- No lazy iterators that hold storage/git handles open (§5.6).
- Writes stay 1:1 with upstream (§5.9). Sugar may rename or default
  arguments, but one Python call = one upstream write.
- abi3 / CPython ≥ 3.9 support is preserved.
- No async, no pickling, no network surface without a named consumer.

## Observed friction (verified against the alpha-rc build)

| # | Problem | Evidence |
|---|---|---|
| P1 | Values have identity equality only: two loads of the same commit are `!=` and hash differently (they *can* be dict keys / set members, but only by identity) | `repo.commit(x) == repo.commit(x)` → `False` |
| P2 | Path parameters reject `pathlib.Path` | `Profile.init(Path(...), ...)` → `TypeError` |
| P3 | Two timestamp units in raw `int`s | Commit/RawOp = epoch **s**; Comment/Review/Revision/RoutingEntry = epoch **ms** |
| P4 | String-enum values typed as plain `str` | `Issue.state`, `Patch.state`, `ProjectDoc.visibility`, `PolicyStore.seed(scope=)`, `set_*_policy(policy=)` |
| P5 | Missing `__repr__` on collaboration values | `repr(comment)` → `<radiclepy.Comment object at 0x…>` (also `Review`, `Revision`) |
| P6 | Long positional same-typed `str` arguments, easy to swap silently | `init_project(…, name, description, default_branch)`; `Patches.create(title, description, target, base, head)` |
| P7 | Same mistake, different exceptions | write via read-only `Issues`/`Patches` → `ValueError`; via read-only `PolicyStore` → `StorageError` |
| P8 | `PolicyStore`/`RoutingStore` keep a SQLite connection open until GC | no `close()`, no `with` |
| P9 | Stringly lifecycle calls | `issue.lifecycle("closed", "solved")` |
| P11 | Cheap container protocols missing | `rid in storage` → `TypeError`; `Tree` has `__len__` but no `__iter__` |
| P12 | Ceremony: `Storage.from_profile(profile)` although every example treats storage as part of the profile | README quick start |

(P10, automatic history paging, is deferred; see below.)

## Patch 1: low-risk friction fixes (non-breaking)

1. **`PathLike` inputs (P2).** Every `path: &str` parameter becomes `PathBuf`.
   PyO3 0.23 resolves the argument with `os.fspath` and then accepts only
   `str` (`conversions/std/osstr.rs`), so the stub type is
   **`str | os.PathLike[str]`** and bytes paths stay unsupported (`TypeError`).
   Return types stay `str`. That is a normal Python API, and changing them would
   create migration work without fixing an observed problem.
2. **`Literal` annotations (P4).** Stub-only, with no runtime change. Add a
   behavioural test asserting that returned runtime values are members of the declared
   `Literal`s, because stubtest cannot check returned values.
3. **Collaboration reprs (P5).** `Comment`, `Review`, `Revision`, in the
   `Issue`/`Patch` style.
4. **`Profile.storage` property (P12).** Returns `Storage.from_profile(self)`;
   the classmethod stays as the low-level form.
5. **Issue lifecycle sugar (P9).** `IssueMut.close(reason="unspecified")` and
   `IssueMut.reopen()`, each exactly one `lifecycle` write (asserted via
   `Raw.history` length). `lifecycle` stays.
6. **Cheap protocols (P11).** `Storage.__contains__(rid)` → `contains`,
   `Tree.__iter__` → `entries`. No `__len__`/`__iter__` on COB stores: they
   would do I/O, and `bool(store)` would silently query storage.

**Done:** `test_api.py` member lists; tests for PathLike (`str`, `Path`,
rejected bytes), reprs, sugar op counts and the protocols.

## Patch 2: snapshot value semantics (designed explicitly)

Changes existing behaviour: equal snapshots will collapse into one set/dict entry.

Contract:
- **Immutable snapshots** (`Commit`, `Tree`, `TreeEntry`, `TagInfo`,
  `HeadInfo`, `ProjectDoc`, `HistoryPage`, `RemoteInfo`, `SetHead`,
  `SignedRefs`, `Issue`, `Patch`, `Comment`, `Review`, `Revision`, `RawOp`,
  `CobRef`, `CobManifest`, policy/routing infos): **structural** equality over
  all fields, including nested contents (an `Issue` equals another only if its
  comments are equal too).
- **Handles** (`Profile`, `Storage`, `Repository`, stores, `*Mut`, `Signer`,
  `Home`): identity equality, unchanged.
- **Hash with equality, per class.** Defining `__eq__` without `__hash__`
  makes CPython set `__hash__ = None`, so the class would become *unhashable*.
  That breaks more than identity hashing does. Every class given `eq` must
  therefore also get `hash` (PyO3 requires `frozen` for that). A class that
  should not be hashable keeps identity semantics and gets no `eq`.
- Getters already return fresh list copies; tests prove that mutating a returned
  list changes neither equality nor hash.

## Patch 3 (when needed): deterministic store closure (P8)

Separate from Patch 1: the backend gains a closed state, and every accessor
must check it.
- `close()` is idempotent; operations after close raise the store's domain error
  (`PolicyError` / `DatabaseError`).
- `__enter__` returns `self`; `__exit__` closes and **does not suppress** the
  body's exception. A closed handle cannot be re-entered.
- `path` and `repr` stay readable after close.
- No transaction semantics are implied.

Trigger: a consumer needing deterministic connection lifetime (e.g. long-
running services that open many stores).

## Patch 4 (when needed): datetime views (P3)

Implemented in the **Python layer**, because PyO3 0.23's `PyDateTime` types are
unavailable under abi3 (`types/mod.rs`, `cfg(not(Py_LIMITED_API))`).
- Comment/Review/Revision (`created_at`), RawOp (`created_at`), RoutingEntry
  (`seen_at`): **aware UTC**.
- Commit `authored_at`/`committed_at`: **aware fixed-offset** datetimes using
  the commit's own offset.
- Missing routing timestamp → `None`.
- Millisecond values converted exactly as `EPOCH_UTC + timedelta(milliseconds=ms)`,
  never through float division.
- Out-of-range values raise `OverflowError` (documented); the raw ints remain
  available and are the lossless contract.

## Patch 5: one small breaking cleanup

README already says APIs may change during alpha. Land it in one patch with a
CHANGELOG migration table.
1. **Keyword-only arguments (P6).**
   - `init_project(storage, signer, path, *, name, description, default_branch, visibility="public")`
   - `Patches.create/draft(title, description, *, base, head, target="delegates", labels=None)`
     (`"delegates"` is upstream's `MergeTarget::default()`).
   - `PatchMut.update(description, *, base, head)`.
2. **`ReadOnlyError(Error)` (P7)** for writes through any read-only handle
   (COB stores and `PolicyStore`).

## Patch 6: tooling

- `mypy.stubtest` alongside the existing AST check (signatures, dunders). It
  does not prove returned values match `Literal`s; Patch 1's behavioural test does.
- `pyright --verifytypes radiclepy` as an extra annotation-completeness
  check, not a target.
- A pytest that extracts the README's fenced quick-start block and executes it
  (no test runs it today).

## Deferred / not doing

- **Automatic history traversal** (`iter_history`). Offset paging re-walks
  from the head on every page (quadratic for full traversals), and `history()`
  clamps to 1,000, so a generator would have to advance by the number of results
  actually returned. Pinning the head stops ref movement but is not a transactional
  snapshot. Revisit when a consumer needs it.
- **`pathlib.Path` return types**: no demonstrated problem.
- **Generalized `NotFoundError`**: it crosses several upstream error
  families and changes what existing handlers catch. If ever done, keep the
  repository vs. git-object distinction and map structured variants only.
- **Removing `Issue.comment_count`/`comment_ids`**: redundant but convenient;
  churn for no gain.
- `NewType` identifiers, `__getattr__` delegation on `*Mut`, merging
  `open`/`open_as`, dataclass rewrites, pickling, async.

## Sizing

| Patch | Size | Risk |
|---|---|---|
| 1 | ~1 day | Low |
| 2 | ~1–2 days (nested derives, contract tests) | Medium: behaviour change |
| 3 | ~1 day when triggered | Medium: lifecycle state across all accessors |
| 4 | ~½–1 day when triggered | Low–medium: conversion edge cases |
| 5 | ~½ day + migration notes | Medium: breaking but mechanical |
| 6 | ~½ day | Low |
