# radicle-sdk changelog

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
