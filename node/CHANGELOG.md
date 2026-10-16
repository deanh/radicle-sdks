# radicle-sdk changelog

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

### Still pending for alpha polish

- Public handle constructor cleanup and negative consumer typing tests.
- Private project initialization and a self-contained, executable README example.

These are a separate follow-up patch. Bare checkout, additional Python APIs,
publication/prebuilt distribution, and additional supported platforms are deferred.
Historical T0 evidence remains in the repository's `FINDINGS-TS-T0.md`; debug-build
results there do not establish optimized-build behavior or performance.
