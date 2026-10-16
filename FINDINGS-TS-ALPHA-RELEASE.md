# TypeScript alpha release polish: Patch A

Issue `fdcfc02`, based on merged `main` at `399d6c4`. This patch covers release
builds, coordinated identity/version/changelog, and an honest platform claim only.
Public constructor cleanup, private initialization and the executable README are
Patch B, to follow after A lands. No Python implementation or dependency changes.

## Distribution contract

- npm package/addon: `radicle-sdk`; private Rust crate: `radicle-sdk-native`.
- Both package versions: `0.1.0`. An exact Git commit identifies each candidate;
  no registry publication, release tag or portable prebuilt binary is implied.
- Initial native support: Linux x86_64 (GNU/glibc) only. macOS, Windows, musl and
  other architectures remain unvalidated; Bun Git prepare remains best effort.
- `prepare` runs the ordinary build, which cleans generated `node/dist`, builds
  with `napi build --release ... -- --locked`, generates native declarations and
  compiles the TS wrapper. Cleaning prevents legacy debug/T0 addons from leaking
  into packs made from existing checkouts. Lockfile edits only rename/version
  the local packages; dependency versions are unchanged.

## Fresh optimized-build evidence

Local Linux x86_64, Node 26.12.0, Rust 1.91.0, separate Cargo target directory:

- Cargo reports `release` profile `[optimized]`; the generated GNU/Linux addon
  was 9,298,248 bytes locally. This is not a cross-platform size guarantee.
- All 26 Node tests pass against that addon, including worker/synchronous panic
  containment, native SSH-agent signing, peer timestamps and cross-worker writes.
- All 4 Rust tests pass with `cargo test --release --locked`.
- Explicit `cargo rustc --release ... -- -C panic=abort` fails with the required
  `radicle-sdk requires panic=unwind` diagnostic. CI repeats this negative gate.
- TypeScript consumer checks, Rust 1.91/stable Clippy and native Rust fmt pass.
- All 193 Python tests pass; Python sources/manifests remain unchanged.
- A local pack dry-run after the clean build contains exactly the renamed release
  addon, not the old T0 debug addon, and includes `node/CHANGELOG.md`.

## Exact-commit gates

After committing, run `node/scripts/git-install-smoke.sh` with the full SHA:

- `npm11`: npm 11.20.0, without npm 12's Git approval option.
- `npm12`: npm 12.2.0 with `--allow-git=root`.
- `pnpm`: pnpm 12.11.2 with exact-source build approval.

The smoke verifies installed version/addon metadata, release-build configuration,
and all three local workflows through installed package exports. Builds use fresh
package-manager caches/stores and may reuse compiled Cargo dependency artifacts.
These are local `git+file://` checks, not remote transport or remote-CI evidence.
Historical T0/debug results are retained separately and do not establish release
behavior. No scheduler or latency guarantee is introduced by optimization.
