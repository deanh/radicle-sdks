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
