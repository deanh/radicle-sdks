# radicle-sdk changelog

## Non-blocking profile setup (unreleased)

- Add `Profile.loadAsync()` and `profile.signerAsync()` on native workers;
  retain synchronous setup methods with their existing error/environment
  semantics. No profile switching, signer cache or in-flight cancellation.

## Working-copy discovery (unreleased)

- Add asynchronous profile-free `projectAt(path, { remoteName? })` with readonly
  `ProjectLocation` metadata, parent/worktree discovery and explicit named
  fetch-remote selection. No Jujutsu, storage lookup or identity verification.

## Consumer mutation guidance (unreleased)

- Clarify that native write locks do not cover prior application reads when
  replacing label/assignee sets; recommend a consumer-owned full read/modify/write
  queue and checking observed reviews, not treating receipts as applied verdicts.
  Existing mutation behavior is unchanged.

## Plain Git ancestry (unreleased)

- Add asynchronous `Repository.isAncestor(ancestor, descendant)`, with validated
  commit endpoints, reflexive equality and propagated Git errors through
  upstream's native graph query. No COB causal range API is added.

## Signed refs at an explicit tip (unreleased)

- Add `Repository.signedRefsAt(nodeId, oid)` using upstream verification and replay
  protection, returning existing frozen `SignedRefs` snapshots. Unreadable or
  invalid sets reject `GIT`; no fallback to current refs or empty results.

## Events discovery primitive (unreleased)

- Add `Repository.namespaceTips()` and frozen `NamespaceTip` snapshots for sorted
  sigrefs targets without loading signed sets or objects. This is a read-only
  building block, not a watcher or an operation feed.

## Python capability parity: raw COB inspection (unreleased)

- Add `Raw.open`, typename/entity/ref enumeration and deterministic causal history,
  deduplicated across tips and excluding identity/related links from traversal.
- Add frozen raw entry/ref/manifest DTOs with independent action bytes; raw seconds
  and schema versions are exact BigInts. `actionsJson()` preserves unsafe integer
  tokens as BigInt; decimal/exponent tokens remain finite IEEE-754 numbers.
- Preserve raw data when JSON decoding fails; reject invalid JSON/UTF-8/surrogates
  and float overflow without adding a dependency or a custom JSON scanner.
- Carry patch-2 review nits: isolated policy worker coordinator, expected busy-test
  duration comment, scope/policy parameter names, API wording and writable-factory
  partial-effect comment. No patch-2 behavior expansion or new synchronization.
- Complete the baseline Python-to-Node member inventory and cumulative local
  conformance audit. Final approval/merge remains separate; no registry/platform
  or upstream-completeness claim is introduced.

## Python capability parity: policy and routing (unreleased)

- Add async raw policy stores with follow/seed/block writes and read-only lookup,
  listings and fuzzy alias search; open/drop a connection per worker operation.
  Read-only factories/reads never create or repair missing databases. Add `POLICY`
  and `DATABASE` operational error codes and profile convenience factories.
- Add read-only routing inventories, entries, counts and last-seen timestamps over
  upstream's shared thread-safe database. Valid times outside the Date range are
  `null`; negative/invalid stored times remain decoding failures.
- Carry patch-1 review nits: upstream DID and SetHead helpers; `INVALID_DATA` for
  unrepresentable stored paths with tests/docs; handle member/import ordering;
  comments explaining snapshot count conversion; primitive-only JSON reviver
  source notes for future raw decoding. No raw COB API is added yet.
- No dependency refresh, networking, node control, automatic retry or merge of
  the policy/routing databases with repository transactions.

## Python capability parity: repository and utilities (unreleased)

- Add repository description, tags, revision parsing and signed-remote inspection;
  include identity version, quorum threshold and commit/blob IDs.
- Add explicit canonical branch/identity-head and remote identity-root writes,
  under the existing repository lock without implicit signing.
- Add opt-in bare checkout and synchronous upstream-backed public ID validators.
- Add async home creation/loading/resolution, profile home and alias facades,
  explicit storage opening/path access and signer DID metadata. Alias reverse
  lookup preserves special keys such as `__proto__` safely.
- Add typed-store RID/count/emptiness, issue comment count/IDs and patch revision
  count. Preserve upstream read-only operations and redaction counting semantics.
- Complete parity patch 1; policy/routing and raw COB inspection remain deferred.
  No upstream/dependency change, publication or platform expansion.

## Assessment fixes (unreleased)

- Lower the Node floor from 26.12.0 to 26.5.0, the oldest release the suite is
  run on; nothing in the SDK needs a later version.
- `TreeEntry.kind` is declared and produced as exactly `'blob' | 'tree' | 'commit'`
  (optional); the unreachable `'tag'`/`'any'` members are gone.

## Native module alignment (unreleased)

- Split the mixed native entry point into profile, project, repository, issues,
  error and argument-parser modules, retaining existing patch ownership.
- Keep detached snapshots domain-local, and worker/panic containment, repository
  locks, timestamp conversion and native probes in binding-specific modules.
- Preserve native declarations/exports, validation and lock order, snapshot timing
  and public wrapper APIs. No dependency or shared-core changes.

## Workspace and package layout (unreleased)

- Join the root Rust workspace and lock without changing Node's existing package
  selections/features; inherit the exact Radicle pin and explicit indexmap std.
- Ship built output, language docs and licenses, not Rust sources/manifests.
  Git `prepare` still builds from the complete clone; packed consumers load the
  addon without compiling Rust. Add an independent tarball consumer gate.
- Rename CI to `node.yml`, include shared workspace/fixture inputs in its filters,
  and retain release/panic/typing/exact-commit source-install checks.
- No public API, native domain-module refactor or shared Rust core is included.

## Documentation ownership (unreleased)

- Split the public contract into the shipped `node/API.md`; keep installation and
  the unchanged executable quick start in `node/README.md`.
- Shared architecture, workflow semantics, SDK differences and development gates
  now live under repository `docs/`. No public API or native build changes.

## 0.1.0 — alpha (release preparation)

Exact-commit Git installation identifies alpha candidates; this is not an npm
registry release. Initial support is Linux x86_64 (GNU/glibc), Node >=26.12.0,
Rust >=1.91, and Git/C build tools. Other native platforms are not yet validated.

### Included

- Local signed project initialization, fork and checkout; issue and patch workflows.
- Async storage/repository reads and explicit ref signing, generated TypeScript
  declarations, frozen detached DTO containers and independent mutable byte copies.
- Worker panic containment and shared repository/signing locks for local writes;
  no networking, commit import, scheduler, or shared Python/TS core.
- Optimized release builds for source installs, including Git dependency `prepare`;
  clean generated output first to avoid packing stale debug/T0 addons.
- Coordinated `radicle-sdk` npm/addon identity and `radicle-sdk-native` Rust crate,
  both versioned 0.1.0; renamed TypeScript alpha CI workflow.
- Exact-commit npm 11/npm 12/pnpm install gates. Bun Git prepare remains best effort
  and is known to fail to install the development build tools.
- Private handle constructors, no public native conversion factories, and direct
  JavaScript construction guards; positive/negative consumer declaration tests.
- `initProject` supports `visibility: 'private'` (empty upstream allowlist),
  retaining public initialization by default and rejecting invalid visibility.
- A self-contained README quick start covering all three workflows, extracted
  from the actual fence and executed through local and Git-installed exports.
- Installed consumer typing gates; requirement-only Node wording and an explicit
  warning that first-time native release compilation can take minutes.

Bare checkout, additional Python APIs, publication/prebuilt distribution, and
additional supported platforms remain deferred.
Historical T0 evidence remains in `docs/archive/FINDINGS-TS-T0.md` in the
repository; debug-build results there do not establish optimized-build behavior
or performance.
