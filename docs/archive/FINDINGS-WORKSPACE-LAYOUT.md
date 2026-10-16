# Architecture layout: dependency alignment and workspace evidence

Tracking issue: `edcbbd7`. Base: approved docs head
`bca68ce536b869d2d23daab9b09fd5ecb3684dc3`. This is review evidence, not a new
platform-support claim or a replacement for the current development guide.

## Dependency decision before moving files

The two original locks selected different versions for 31 shared package names.
A single workspace lock is therefore not a dependency-neutral rename. Preserve
Node's existing newer selections rather than downgrade Node, retain Python-only
packages (including PyO3 0.23.5), and isolate this alignment in its own commit.
No version was selected outside the union of the existing locks. Radicle remains
exactly 0.25.1; Rust minimum remains 1.91. The standalone Node lock is unchanged
in the alignment commit.

The Python lock changes as follows (for `syn`, only the 3.x selection changes):

| Package | Previous Python | Existing Node / aligned Python |
| --- | --- | --- |
| `bitflags` | 2.13.1 | 2.13.2 |
| `cc` | 1.4.5 | 1.8.0 |
| `cfg-if` | 1.0.4 | 1.0.5 |
| `defer-heavy` | 0.1.0 | 0.1.1 |
| `equivalent` | 1.0.2 | 1.0.3 |
| `find-msvc-tools` | 0.1.12 | 0.1.14 |
| `js-sys` | 0.3.104 | 0.3.106 |
| `lazy_static` | 1.5.0 | 1.5.1 |
| `libc` | 0.2.189 | 0.2.190 |
| `libz-sys` | 1.1.29 | 1.1.30 |
| `rustix` | 1.1.4 | 1.1.5 |
| `schemars` | 1.2.2 | 1.2.3 |
| `schemars_derive` | 1.2.2 | 1.2.3 |
| `serde_json` | 1.0.151 | 1.0.154 |
| `siphasher` | 1.0.3 | 1.0.4 |
| `smallvec` | 1.16.0 | 1.16.3 |
| `sqlite` | 0.37.0 | 0.37.4 |
| `sqlite3-src` | 0.7.0 | 0.7.1 |
| `sqlite3-sys` | 0.18.0 | 0.18.2 |
| `syn` | 3.0.5 | 3.0.9 |
| `thiserror` | 2.0.20 | 2.0.21 |
| `thiserror-impl` | 2.0.20 | 2.0.21 |
| `tinyvec` | 1.13.2 | 1.13.3 |
| `unicode-ident` | 1.0.24 | 1.0.27 |
| `wasm-bindgen` | 0.2.127 | 0.2.129 |
| `wasm-bindgen-macro` | 0.2.127 | 0.2.129 |
| `wasm-bindgen-macro-support` | 0.2.127 | 0.2.129 |
| `wasm-bindgen-shared` | 0.2.127 | 0.2.129 |
| `zerocopy` | 0.8.56 | 0.8.63 |
| `zerocopy-derive` | 0.8.56 | 0.8.63 |
| `zeroize` | 1.9.0 | 1.9.1 |

The graph also drops unused `tinyvec_macros` and adds Node's existing `foldhash`
selection. Compare package/version/source keys, not just package-name dictionaries
which can hide multiple versions. Feature trees were captured with Cargo 1.91
for each adapter independently, before alignment and again before/after layout.

### Explicit indexmap feature requirement

The first isolated Python build after alignment failed upstream with missing
IndexMap/IndexSet default hasher generics. Python's old graph had incidentally
enabled `indexmap/std`; the newer graph did not. Add the same explicit std-enabled
indexmap dependency already used by Node. Do not rely on a combined workspace
build to hide a missing adapter feature. This is a build-graph correction, not a
new Python API or upstream patch.

### Alignment validation before layout

On local Linux x86_64 GNU/glibc, using Rust 1.91 and CPython 3.13:

- fresh helper extension build, 200 Python tests with required digest parity;
- installed stubtest, strict mypy, pyright consumer checks and 100% verifytypes;
- Clippy with and without `test-helpers`, warnings denied.

Optimized and exact-commit installed-artifact gates apply to the final layout
candidate, not to this intermediate debug test result.

## Sdist feasibility experiment

Before moving tests/workflows, a throwaway workspace plus Python-crate move was
built with maturin 1.15.0. Maturin rewrote sdist workspace membership to
`python/native`, retaining inherited workspace dependencies. After deleting the
experimental checkout, the unpacked sdist built offline with Rust 1.91 and its
installed helper-free wheel passed production/workflow smoke checks.

A strict initial `--locked` build failed because the rewritten workspace still
had Node-only selections in the bundled lock. Normal offline building pruned 19
unused selections and added no package versions. This proves source closure,
not immutable sdist-lock support or vendoring of registry dependencies. The final
candidate must repeat offline building and compare retained package selections;
Git/workspace builds continue using `--locked`.

The experiment's Python-preserving dependency seed was not adopted: it would
have downgraded Node. The final candidate instead uses the explicit alignment
above and must validate that graph independently.

## Maintained layout and artifact validation

The layout commit moves Python's Rust crate and private test suite into its SDK
folder, makes the root a resolver-3 virtual workspace, and removes the standalone
Node lock. The Python package version/import and Node package/addon identities
are unchanged. Modules are not reorganized in this patch.

- Node's per-package feature tree is byte-identical before/after workspace entry.
- Python's tree after explicit alignment changes only its crate's filesystem path
  during the layout move. All package selections are from the original lock union.
- Shared `git`, `signed_refs`, `project_fixture` and `add_commit` functions move
  unchanged into `tests/sdk_fixture.py`; root examples import it directly.
  All four cross-SDK example tests pass with `python/tests/` temporarily absent.
- Workflows become `python.yml` and `node.yml`, retaining installed/artifact,
  minimum/stable Rust, panic and source-install gates. Node filters include both
  SDK directories and shared manifests/fixtures.
- npm packing contains built output, language docs and licenses, with no Rust
  sources/manifests. A separate tarball consumer test installs with invalid Rust
  tool paths, runs actual installed workflows/README, and checks consumer types.
- Fresh optimized Node output is built in a new target directory with Rust 1.91:
  30 Node tests, consumer type checks, four native release tests and abort-mode
  rejection pass. Clippy passes per adapter on 1.91 and stable; Node fmt passes.
  Pre-existing unrelated Python formatting is not rewritten.
- Fresh Python helper build on CPython 3.13: 202 tests pass with required Node
  parity. An MSRV-built abi3 release wheel installs on CPython 3.9 and 3.14:
  each has 89 passed/113 helper-dependent skips, production/workflow smoke,
  stubtest/mypy/pyright and 100% annotation completeness. Local wheels are
  manylinux_2_39, not evidence for CI's manylinux_2_28/macOS targets.
- Installed pyright completeness is checked outside the checkout with the
  explicit consumer interpreter: an ambient source/config lookup returned no
  package even though runtime imports and `py.typed` were present. CI now uses
  the independent installed lookup rather than trusting the caller's context.
- The actual aligned-layout sdist builds offline on Rust 1.91 in a clean consumer.
  It prunes 19 Node-only lock selections, adds none, and passes installed
  production/workflow smoke. CI caches dependencies first and checks this same
  no-new-selection invariant. Source closure is not a promise of all-vendored
  dependencies or strict immutable sdist locks.

Candidate-specific exact-commit Git-install evidence is recorded in the patch
and issue discussion after committing. Local checks are not remote CI results.
