# Releasing the SDKs

Releases are built on the maintainer's own machines, recorded on this Radicle
repository as a signed release object through
[rad-artifact](https://radicle.network/nodes/hdh.radicle.garden/rad%3Az4VYyJ9KuwMNkXGQnmKuGPGKw3inv),
and published to PyPI (`radicle-py`) and npm (`radicle-sdk`). The release
object is the receipt: bound to the annotated tag, it records every artifact
by content hash together with `MANIFEST.json` (source commit, targets,
floors, toolchain, per-file digests), and the bytes are seeded from the
maintainer's artifact node. The registries are the channel: `pip install
radicle-py` and `npm install radicle-sdk`. Publishing uploads exactly the
bytes the receipt records, and verification checks that the registries serve
those bytes before installing from them.

Tracking: issue `a17f70b` (plan), `fdcfc02` (alpha release). Release 0.1.0
predates the registry step and is available only through rad-artifact.

## Targets

| Release | Platforms | Floors |
| --- | --- | --- |
| 0.1.0 | Linux x86_64 only | glibc 2.28, CPython 3.9 (abi3), Node 26.5 |
| next | Linux x86_64 and macOS arm64 | as above; macOS 11.0 for the wheel, 13.5 for the Node addon (Node 26's own minimum) |

Linux aarch64 would be one more container run but needs an ARM machine to
verify the installed artifact before it is published.

## Artifacts per release

- `radicle_py-<version>-cp39-abi3-manylinux_2_28_x86_64.whl`
- `radicle_py-<version>-cp39-abi3-macosx_11_0_arm64.whl` (releases that
  cover macOS)
- `radicle_py-<version>.tar.gz` (sdist, builds from source anywhere with Rust
  1.91+ and a C toolchain)
- `radicle-sdk-<version>.tgz` (one npm tarball with every released addon
  inside, selected by the loader at import time; installs with no Rust, C
  compiler or Git)
- `MANIFEST.json`: source commit and date, version, targets, toolchain
  identity, the SHA-256 of each file with the targets it serves, and under
  `platforms` the floors, toolchain and audit lines of every platform built
  on its own hardware; registered as an artifact so consumers can check the
  files they downloaded against it. It carries no build timestamp, so a
  rebuild that reproduces the same artifact bytes reproduces the same
  manifest
- `SHA256SUMS` (local convenience, not registered)

Both packages ship `THIRD-PARTY-NOTICES.md` next to the SDK licence files: the
licence and copyright texts of the statically linked libgit2, SQLite and zlib
and of every Rust crate compiled into the native module, generated from
`Cargo.lock` by `scripts/release/third_party_notices.py` for the release
target. Crates are classified as runtime (compiled into the module) or
build-time only; compound licence expressions keep their AND terms; every
licence file a crate ships is reproduced. A runtime crate whose registry
source carries no licence text must have a reviewed override under
`scripts/release/notices/<crate>/` with a `SOURCE` file naming where the text
came from, or generation fails. `tests/test_third_party_notices.py` checks the
committed file against `Cargo.lock`.

## Process

Prerequisites on the Linux build host: Docker, Node 26.5+, uv, `objdump`,
`unzip`, `curl`, the `rad` CLI with this profile as a delegate, `rad-artifact`
and `rad-artifact-node` (`cargo install radicle-artifact radicle-artifact-node`),
a PyPI token scoped to the `radicle-py` project, and an npm login with
publish rights on `radicle-sdk` (`npm login`; 2FA may prompt at publish time).
The PyPI token is read from `UV_PUBLISH_TOKEN` in the environment or, when
that is empty, from a `.env` file at the repository root holding
`UV_PUBLISH_TOKEN=pypi-...`. The file is parsed, never executed or printed,
and the publish script refuses it unless git ignores it (`/.env` is in
`.gitignore`) and only its owner can read it (`chmod 600 .env`). Local builds cannot use PyPI trusted publishing or npm
provenance, which need a hosted CI identity; the rad-artifact receipt is the
provenance record instead.

1. **Prepare.** On a branch: set the version in `pyproject.toml`, the two
   native `Cargo.toml` files and `package.json` (all four must agree), fold the
   "(unreleased)" changelog sections under a dated version heading in
   `python/CHANGELOG.md` and `node/CHANGELOG.md`, regenerate the notices with
   `scripts/release/third_party_notices.py --write` if `Cargo.lock` changed,
   submit as a Radicle patch, review, merge.
2. **Tag.** On the merged commit:

   ```sh
   git tag -a v<version> -m "radicle-py and radicle-sdk <version>"
   git push rad v<version>
   ```

   The tag is annotated so the release object records both the commit and the
   tag. Tag names are not resolvable by `rad-artifact` unless they are
   canonical references, so the scripts bind by the tag's object ID.
3. **Build.** With a clean checkout of the tagged commit:

   ```sh
   scripts/release/build-linux-x86_64.sh
   ```

   Compiles both native libraries inside the official `manylinux_2_28` image
   with a pinned Rust toolchain, generates the JavaScript loader on the host
   and produces the artifacts under `release/<version>/`. Every gate is fatal:
   clean tree and one version in all four manifests; current notices file;
   each native library, extracted from the final wheel and tarball, audits as
   x86-64 with a glibc floor of 2.28 or lower and no dynamic library beyond
   the base set (`scripts/release/audit-elf.sh`); notices present in wheel,
   sdist and tarball; the exact release tarball passes the packaged consumer
   smoke with no compiler; the wheel installs wheel-only into a clean
   environment at the expected version and passes the wheel smoke. Only then
   is `MANIFEST.json` written.
4. **Register and seed the receipt.**

   ```sh
   rad-artifact node start
   scripts/release/register.sh <version>
   ```

   Preflight before anything is registered: the manifest names this version
   and exactly the expected files, every file matches its manifest digest and
   `SHA256SUMS`, the local `v<version>` is an annotated tag whose object equals
   the one on the rad remote, and that tag's commit equals the manifest's
   source commit. Then the release state already on the tag is compared with
   the local build by content hash: an artifact already registered with the
   same name and the same content is resumed, a missing one is registered, and
   a live artifact with the same name but different content, or with an
   unexpected name, stops the script before any write. Published versions are
   immutable: a changed build means a new version, never a second file under
   the old name. After preflight each file and the manifest are registered
   against the tag object and seeded from this host's node, which adds a
   `radiroh://` location. Interrupted runs are resumed by rerunning. The node
   must stay running for peers to fetch; the seeded bytes are kept under
   `~/.radicle/artifacts/`.
5. **Verify from a download**, on the build host and on any other machine
   that seeds the repository:

   ```sh
   scripts/release/verify-installed.sh <version>
   ```

   Requires exactly one release bound to the tag whose non-redacted artifacts
   are exactly the expected names for that version, downloads them through
   `rad-artifact`, verifies each against the release object and against the
   downloaded manifest's digests, installs the wheel wheel-only into a fresh
   environment and the npm tarball into a fresh consumer with scripts disabled
   and no compiler, and checks that both installed packages report the
   requested version. A listing for another version, an extra artifact, or an
   sdist fallback cannot pass.
6. **Publish to the registries.**

   ```sh
   scripts/release/publish-registries.sh <version> --dry-run
   scripts/release/publish-registries.sh <version>
   ```

   Preflight, all before any upload: the release directory matches its
   manifest; the tag is the one on the rad remote and points at the
   manifest's source commit; the receipt is registered, meaning exactly one
   release object on the tag whose live artifacts are exactly the manifest's
   files plus `MANIFEST.json` with the content hashes of the local files; and
   the registries hold nothing conflicting. A file already on PyPI or npm
   under this version with the same digest is skipped, so an interrupted run
   is resumed by rerunning; one with a different digest, or a PyPI file this
   release did not build, stops the script, because published files are
   immutable and the fix is a new version. Credentials are checked only for
   what is still to upload. Then `uv publish` uploads the pending wheels and
   sdist (with `--check-url` as a second guard against duplicates) and
   `npm publish` uploads the exact tarball.
7. **Verify from the registries**, on each released platform:

   ```sh
   scripts/release/verify-registries.sh <version>
   ```

   Fetches and verifies the receipt's manifest through `rad-artifact`, then
   requires PyPI to hold exactly the manifest's Python files with the
   manifest's digests and downloads each to check the served bytes, requires
   npm to report the manifest's tarball integrity and fetches the tarball to
   check its digest, and only then installs this platform's wheel from PyPI
   (wheel-only, no cache, no local files) and the package from npm (scripts
   disabled) into clean environments, checking the installed versions.
8. **Record.** Comment on the release issue with `SHA256SUMS`, the release
   object ID and the verification results per machine and per channel.

## Adding macOS arm64

The Mac builds its own pieces natively and the Linux host assembles the
release, so there is one manifest, one tarball and one registration.

Prerequisites on the Mac, once: Xcode command line tools; rustup with the
pinned toolchain and target (`rustup toolchain install 1.91.0` and
`rustup target add aarch64-apple-darwin --toolchain 1.91.0`); Node 26.5+;
uv with the project synced (`uv sync --locked`); a build of the Node package
(`npm ci --ignore-scripts`). Docker and rad-artifact are not needed to build;
rad-artifact and a seeded copy of the repository are needed only to run the
download verifier there.

1. On the Mac, at the tagged commit with a clean tree:

   ```sh
   scripts/release/build-macos-arm64.sh
   ```

   Builds the wheel (deployment target 11.0, maturin's arm64 default) and the
   addon (deployment target 13.5, Node 26's minimum), audits each against its
   floor with `scripts/release/audit-macho.sh` (arm64, `LC_BUILD_VERSION`
   minimum, only `/usr/lib/libSystem`, `/usr/lib/libiconv` and system
   frameworks, ad-hoc signature
   verifies), runs the packaged consumer smoke on a tarball carrying this
   addon and the wheel-only install smoke, and writes
   `release/incoming/darwin-arm64/` with both files and `AUDIT.json` (source,
   version, floors, toolchain, digests, audit lines).
2. Copy that directory to the Linux host as `release/incoming/darwin-arm64/`
   (for example with `scp -r`).
3. On the Linux host, run `scripts/release/build-linux-x86_64.sh` as usual.
   It checks the receipt with `scripts/release/platform_receipt.py`: it must
   name exactly the macOS wheel and addon for this version, with digests,
   sizes and passing audit lines, for this source commit, and nothing else
   may be staged beside them. The script copies those two files by name,
   folds the wheel into the release and the addon into the npm tarball,
   compares the bytes inside the final tarball and release directory back to
   the receipt's digests, and writes a manifest whose `targets` lists both
   platforms and whose `platforms` section embeds the receipt's floors,
   toolchain and audit lines. Registration then expects both wheels.
4. Verify from a download on both machines with
   `scripts/release/verify-installed.sh <version>`; the script picks this
   machine's wheel from the manifest.

The Mac does not run the full test suite as a release gate; run it there once
per release anyway (`uv run --no-sync pytest` and `npm test`) to catch a
platform-specific bug before users do.

## Installing a release (consumer view)

```sh
pip install radicle-py==<version>
npm install radicle-sdk@<version>
```

To check a registry download against the receipt, compare its SHA-256 with
the `MANIFEST.json` registered on the release object (`pip download` and
`npm pack` fetch the files without installing), or run
`scripts/release/verify-registries.sh <version>` on a machine that seeds the
repository. The files themselves can also be fetched from the receipt:

```sh
rad-artifact list --repo rad:z27dDy6sppkZDG8qYcySVX9Qn5jNg
rad-artifact download <tag-object-id> --cid <cid> --repo rad:z27dDy6sppkZDG8qYcySVX9Qn5jNg
rad-artifact verify <file> --repo rad:z27dDy6sppkZDG8qYcySVX9Qn5jNg
```

`download` needs a running `rad-artifact node`; `verify` reads local storage
only and fails if the bytes match no artifact a delegate registered, or one a
delegate redacted. See `scripts/release/verify-installed.sh` for the exact
commands.

## Later targets and gates

Distilled from the earlier distribution proposal (patch `0a86b28`); these apply
when a second platform is added, starting with macOS arm64.

- **One toolchain per platform, pinned.** Build on the target's own hardware
  with the same pinned Rust and maturin versions as Linux; cross-compilation is
  not a substitute for running the installed artifact natively.
- **Audit before registering.** Linux: `audit-elf.sh`. macOS: `audit-macho.sh`.
  A new platform gets its own audit script with the same shape: architecture,
  declared minimum OS, allowed dynamic libraries, nothing from the build host.
  Set the deployment target explicitly and state it as the floor; a successful
  compile on a newer OS proves nothing about older ones.
- **Same bytes everywhere.** Every verification installs the registered file,
  downloaded through `rad-artifact` or from the registry, never a staging copy
  from the build tree; what the registries serve is compared with the receipt
  before it is installed.
- **No install-time build or download.** A release tarball or wheel must
  install with scripts disabled and no compiler on the path; Git `prepare`
  stays a development path only.
- **Licences.** `THIRD-PARTY-NOTICES.md` is generated from `Cargo.lock`; a
  new platform that pulls in platform-specific crates or links another system
  library must regenerate it and the build gate checks it is current.
- **Node packaging.** One tarball with every addon inside, selected by the
  generated loader at import time. It grows by about 10 MB per platform; move
  to napi platform packages only if that becomes a problem.

## Withdrawing an artifact

`rad-artifact redact <tag-oid> --cid <cid> -m "<reason>"` by a delegate makes
`verify` fail for that file everywhere; the bytes are not deleted. On the
registries, yank the PyPI release and `npm deprecate` the version; neither
removes the files. Publish a fixed version rather than re-registering or
re-uploading different bytes under the same name.
