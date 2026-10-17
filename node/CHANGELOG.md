# radicle-sdk changelog

## Python capability parity (in progress, unreleased)

- Add repository description, tags, revision parsing and signed-remote inspection;
  include identity version, quorum threshold and commit/blob IDs.
- Add explicit canonical branch/identity-head and remote identity-root writes,
  under the existing repository lock without implicit signing.
- Add opt-in bare checkout and synchronous upstream-backed public ID validators.
- This is the first slice of parity patch 1, not a claim of completed parity.

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
