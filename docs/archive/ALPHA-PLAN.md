# Assessment and minimal release plan

## Alpha-RC execution record

The assessment below describes the **pre-RC baseline**. On branch `alpha-rc`,
the bounded core-MVP implementation is complete:

- B1: non-interactive `init_project`, `fork_project`, `checkout_project` crate adapters; explicit metadata, real Git inputs, documented Git executable dependency and partial failures.
- B2: typed immutable issue comments with bodies/authors/thread links.
- B3: mutable/reloadable patches, revision updates, lifecycle, labels/assignees, comments, basic reviews and merge records; typed revisions/reviews and evaluated merge fields.
- A1/A2: updated API/stubs/README, lazy-store contract, runnable examples, encrypted-key tests, signature/value checks, and helper-free installed-artifact domain workflows.
- Distribution scope changed by agreement: **Git URI installation**, pinned to a reviewed commit; **no PyPI publication** or release-version expansion. Package version stays `0.1.0`; Git SHA identifies the RC.
- CI now validates exact-commit Git URI installs, installed-wheel domain workflows on oldest/current Python, and native Intel execution of the universal2 artifact.

Local validation: **179 tests passed** in the helper build; both Clippy feature states passed. Release wheel installed on **CPython 3.9 and 3.14**: complete workflow smoke passed and **66 tests passed / 113 legacy fixture skips** on each. The sdist built and installed on CPython 3.14; workflow smoke passed. Exact-commit Git URI installations of `ea73a08cac90e5d5ec10e44987d22099419f2c14` succeeded in clean environments on **CPython 3.9 (pip)** and **3.14 (uv)**; the complete helper-free workflow smoke passed on both. Metadata checks (`twine check`, no upload) passed for wheel and sdist. Local Rust was 1.97.1; the declared 1.88 MSRV remains a CI gate.

Remaining external verification: run the configured native Linux aarch64/macOS and manylinux_2_28 CI jobs; these are not verified locally. Host Linux wheels are manylinux_2_39 validation artifacts, not universal distribution claims. Live SSH-agent integration remains explicitly unverified. The initial delivery had no remote; the repository has since been initialized in Radicle and submitted as patch `d73ccc6cf80efe8660f9ba33d1997d30540d339f`.

### Review follow-up

- Init preflights any existing `rad` remote before calling upstream; regression tests assert the remote, full local config, tracking refs and stored project survive re-init with different metadata.
- `Patch.latest_revision` identifies the author revision matching summary fields; `version` is documented as all-author revision slots minus one, including redactions, not a list index. Mixed-author coverage exercises review selection via the new ID.
- Branch listings remove exactly one heads prefix. Checkout's canonical content versus push namespace, retained GIL, and raw-sort complexity are explicit.
- Workflow signing checks now read persisted sigrefs and their COB tips without an explicit signing operation.
- Validation after review: **181 passed** with helpers; Clippy passed in both feature states. Clean release wheels on CPython **3.9 and 3.14** passed full workflow smoke and **68 tests / 113 legacy fixture skips** each.

## Baseline recommendation

**Ship a clearly scoped local-data alpha after release validation; do not call it comprehensive crate coverage.** The current implementation is a credible alpha candidate, not merely a feasibility experiment. For a core-workflow MVP, close the project-lifecycle and collaboration gaps below, then stop adding features until consumers need them.

There are two distinct finish lines:

1. **Alpha:** existing repository inspection, profiles, local policy/routing, issues and limited patch creation. Existing Radicle projects are a prerequisite. No new feature family is required to publish this honestly scoped alpha.
2. **Core MVP:** users can initialize a project, fork/check out a locally available project, read issue discussions, and perform a basic patch revision/review/lifecycle workflow through the SDK. This needs bounded additions; finishing the original phase checklist does not provide these workflows.

If only one release is desired, use the core MVP finish line and label that release alpha. Do not wait for every public Rust function to have a Python counterpart.

## Evidence and progress

Reviewed the binding, Python exports/stubs, tests, packaging workflow, historical plan/findings, and the installed crates.io source for **radicle 0.25.1** (not adjacent Heartwood HEAD).

Verified locally:

- `uv run pytest -q`: **168 passed** with the installed test-helper build.
- Clippy with `--all-targets --locked -- -D warnings`: passed both with and without `test-helpers`.
- `uv run maturin build --release --locked`: built a release wheel.
- Clean temporary environment, installed that wheel: `tests/wheel_smoke.py` passed; full pytest invocation yielded **55 passed, 113 skipped**.
- The locally built wheel is **manylinux_2_39 x86_64**, not the promised manylinux_2_28 artifact. It validates the local release surface, not the advertised platform matrix.

Not independently verified here: remote CI results, native aarch64/macOS execution, Python 3.9 compatibility, sdist installation, PyPI ownership/availability or publication. Historical claims in `PLAN.md` are not substitutes for release-run evidence.

### What is already strong

- Exact upstream pin, PyO3/abi3 + maturin packaging, conventional hybrid package.
- Explicit value schemas, byte-preserving Git data, documented units and canonical identifiers.
- Owned handles/reopen-per-operation avoid exposing borrowed Rust/Git lifetimes.
- Error hierarchy and distinction between input validation and operational failures.
- Typed COB adapters instead of an invented generic domain materializer.
- Raw COB causal traversal, deduplication and topological ordering; regression coverage for prior ref-parsing and timestamp mistakes.
- Feature-gated deterministic fixtures absent from the release wheel.
- Stubs, `py.typed`, API-name/kind checks, licenses, changelog, and a substantial build/artifact workflow are already present.

The original implementation phases are substantially delivered. The remaining issue is **which core user workflows the phase plan left out**, plus validating the actual release artifacts.

## Core coverage map

“Core” here means public domain workflows, not every helper, trait, iterator, schema or daemon bookkeeping function. No meaningful single coverage percentage follows from counting exported names.

| Area | Current coverage | Release disposition |
|---|---|---|
| Profile/home/identifiers | Init/load, aliases, signer acquisition and identity | Keep; verify encrypted-key success/failure paths |
| Project identity | Canonical verified project snapshot, delegates/threshold/visibility | Adequate alpha/MVP reads; identity proposal/voting/editing is deferred governance work |
| Storage/Git inspection | List/open/contains, heads, branches/tags, commits/trees/blobs/history, remote ref names | Strong local read surface; remote ref OIDs/validation APIs remain partial |
| Project lifecycle (`rad`) | **No public `rad::init`, `fork`, or `checkout` wrappers**; project creation exists only in test helpers | Principal core-MVP gap |
| Issues | List/get/create, title/description, labels/assignees/state, append comments | **Only comment IDs/counts are readable**, not typed discussion bodies; close this MVP gap |
| Patches | List/get summary and create/draft | `PatchMut` is only a creation snapshot: no get-mut/reload/update/review/comment/merge or lifecycle mutation; principal collaboration gap |
| Raw COB inspection | Types/entities/refs/causal operations with bytes and JSON views | Keep; useful diagnostic escape hatch, not a substitute for typed discussions/reviews |
| Policy | Broad local follow/seed/block policy reads/writes | Keep; document this as local persisted policy, not fetch/connection control |
| Routing | Inventory/entry/count reads | Sufficient local-data scope; no daemon bookkeeping writes needed |
| Addresses | No public address store | Defer; requires a concrete discovery/administration use case |
| Node control | No wrapper for synchronous `node::Node`/`Handle` | Explicitly excluded from local-data alpha/MVP; fetch/announce/connect require a daemon and a separate integration gate |
| Other modules | CLI/web/explorer, caches, notifications, schemas and internal utilities largely unwrapped | Not mandatory SDK domain coverage |

The distinction matters: excluding the daemon implementation does **not** imply the crate lacks node control. Its control client is synchronous and could be wrapped later. A local-only release must not imply SDK-managed network clone/fetch/publish.

## A. Alpha release gates — smallest path to publication

### A1. Make the supported scope truthful

- README opening: alpha, local existing-project inspection and narrow writes; not comprehensive and not a node client.
- Replace the quick-start placeholder workflow (fresh empty profile followed by opening an unknown RID) with an existing-profile/project example that can actually run. Explain how users obtain/populate storage externally.
- Add a short limitations list: no project init/fork/checkout, typed discussion bodies, patch mutation/review, identity governance or node fetch/announce.
- Explain that local policy edits do not themselves perform networking. Repository canonical-ref writes require explicit signing where applicable; upstream typed COB writes have their own signing behavior.
- State current constraints: eager COB lists/history, snapshot semantics, string-only paths, and thread-affine signer/write-store handles (`unsendable`). Do not promise async/thread-safe behavior.
- Mark `PLAN.md` as historical implementation design or refresh its stale status/sketches. It currently mixes Phase-1 status with Phase-6 completion; `API.md` remains the current API contract.

**Done:** a new user can identify what works, what needs external tooling, and what is not supported without reading findings documents.

### A2. Test behavior through the installed release artifact

The 113 skips in a release-wheel test run are fixture-dependent skips, **not 113 failing or absent production features**. But current wheel smoke tests only cover import, version/helper purity and profile/signing. They do not prove shipped repository/COB functionality on each platform.

- Extend artifact smoke with a small helper-free repository fixture and representative storage read, issue write/read and patch create/read. A committed small fixture or a fixture produced separately by the test build is sufficient; never ship test helpers in the production extension.
- Assert persistence after reopening, signed-ref effects of COB writes, and malformed/missing-object errors.
- Add an encrypted profile test using `RAD_PASSPHRASE`, including wrong/missing passphrase behavior. Keep SSH-agent integration optional and document that it was not tested if unavailable.
- Resolve observed contract differences: `Issues.open`/`Patches.open` currently only retain the repository handle, while `API.md` describes opening an upstream store and immediate `CobError` on failure. Specify lazy validation or validate eagerly, and test the chosen behavior.
- Add a focused stub/signature/return-type check for representative APIs. Existing conformance checks mainly validate names and descriptor kinds, not full typing fidelity.

**Done:** native installed-wheel tests exercise the advertised domain operations without rebuilding with `test-helpers`; helper purity and versions still pass.

### A3. Validate artifacts and the Git-URI install flow

- Run the existing workflow and inspect final repaired Linux wheels, macOS wheel and sdist. Do not upload host-built `dist/` wheels as manylinux_2_28 releases.
- Exercise the oldest advertised Python (3.9) and a current Python through installed wheels. macOS universal2 should be executed on both architectures before claiming both are verified; abi3 alone is not behavioral compatibility evidence.
- Verify sdist clean installation, wheel inclusion of stubs/`py.typed`/licenses, and distribution metadata (`twine check`).
- Keep package version `0.1.0` and its Alpha classifier; use the full Git commit SHA to identify/reproduce each RC.
- Install that exact commit via Git URI in a clean environment and repeat the complete helper-free workflow smoke. Document source-build prerequisites and branch-versus-commit pinning.
- No PyPI publication, ownership check, upload, or automatic publisher is required for this alpha.

**Done locally:** release wheel/source installations and domain smoke pass on oldest/current CPython; exact-commit Git installation is verified. Native platform CI is the external gate for platform-specific claims.

No documentation site or new packaging framework is needed.

## B. Bounded core-MVP additions

Implement in order. Each slice includes `API.md`, exports, stubs and tests; there is no separate “typing later” phase. Preserve upstream semantics and existing value/handle conventions.

### B1. Local project lifecycle

Wrap the production `rad::init`, `rad::fork`, and `rad::checkout` operations with path/identifier inputs and owned Python results. Choose one API location rather than duplicating free functions and methods. Return existing value types or identifiers/paths rather than exposing borrowed git2 handles.

- `init`: convert an existing Git repository into a Radicle project; name, description, default branch, supported visibility, storage and signer.
- `fork`: establish the signer's local namespace for an already stored project.
- `checkout`: create a working copy from local storage, retaining upstream remote and bare-mode semantics. This is **not network clone**.
- Use real caller-provided commits; do not promote the deterministic test helper into the public API.
- Account for upstream subprocess dependencies: `rad::init` and `rad::checkout` invoke the **Git executable**. Document runtime prerequisites separately from wheel import prerequisites, including the remote helper needed for subsequent external `git` Radicle workflows. Do not advertise these as pure bundled-libgit2 operations.
- Test existing destinations, absent commits/projects, invalid metadata and failure/cleanup behavior. Preserve/document upstream multi-step partial failures rather than inventing rollback guarantees.

**Done:** through public APIs alone, a temporary Git repository becomes a stored project, can be read and forked, and produces a checkout. No node daemon is involved.

### B2. Typed issue discussion reads

Expose comments as immutable values: ID, author, body, timestamp and reply target, using upstream units/order/redaction semantics. Offer one simple eager retrieval surface consistent with existing stores.

**Done:** create issue → comment/reply → reopen → read bodies/authors/thread links → close/reopen. Raw JSON decoding is not required. Comment editing/redaction/reactions/attachments can remain deferred unless required by the first consumer.

### B3. Basic patch workflow, not every patch action

Reuse the issue-handle pattern: add `Patches.get_mut`, a real mutable/reloadable patch handle, and typed revision/review/discussion reads.

Bound the initial mutations to:

- Edit title/target, replace labels/assignees.
- Add a revision (`update`) with existing base/head commits.
- Ready/unready and archive/unarchive lifecycle transitions.
- Add/read a basic revision comment and review verdict with summary.
- Record a merge via the upstream operation and expose its result. **Recording a COB merge is not performing an arbitrary Git merge**; document prerequisites and let callers supply the commit.

Exclude inline-code review machinery, reactions, attachments, redaction/edit histories and custom transaction batching from this slice. Derive exact inputs/authorization from pinned upstream types before committing to Python signatures.

**Done:** a two-actor test creates a draft with genuinely different base/head commits, promotes it, adds a revision, reads/comments/reviews that revision, records a valid merge, and verifies state and signed refs after reopening. Negative authorization and missing-commit cases raise documented errors.

B1–B3 are the proposed core-MVP boundary. They close normal workflows rather than mechanically wrapping all methods. If the first consumers are strictly read-only, publish A first and deliver these slices incrementally; do not call patch creation alone complete patch support.

## Explicit YAGNI backlog

Not release blockers: address-store management, routing writes/pruning, node control/network operations, identity proposal/voting/private-access administration, notification/cache APIs, all COB actions, generic raw COB mutation, arbitrary Git writes/diffs/merge engines, async wrappers, rich identifier classes, lazy iterators, Windows/musl/new wheel architectures, automatic publication and a hosted documentation site.

Add an item only for a named consumer workflow. Node fetch/announce may become the next priority if consumers need network interaction, but that is an explicit scope change, not a hidden requirement of local alpha.

## Stop rule

Release once the chosen finish line passes its gates. For core MVP, demonstrate these three helper-free workflows:

1. Git repository → Radicle init → inspect/fork → local checkout.
2. Issue create → typed discussion → assignment/labels → close/reopen.
3. Patch draft → ready → new revision → comment/review → merge record.

Keep the upstream pin fixed for this release. No rewrite or broad abstraction work is indicated by the evidence. The next task should be a missing workflow or demonstrated defect, not another expansion of the surface map.
