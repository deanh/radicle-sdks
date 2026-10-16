# Architecture and ownership

The project provides two language bindings to upstream Radicle **`=0.25.1`**,
not a node, network client or replacement Git implementation. Both expose local
signed project, issue and patch workflows. Full API parity is not a goal.

## Ownership rule

**Each SDK owns its implementation, tests and language-specific documentation
under its language directory. Shared material has explicitly named root
directories. Tool-required entry manifests remain at root.**

Directory ownership is not artifact contents: development tests need not ship,
and a source build may require root manifests and shared dependencies.

- `python/`: Python package/stubs, Rust binding, tests, guide, API and changelog.
- `node/`: TypeScript wrapper, native binding, tests, scripts, guide, API reference
  and changelog. Generated `node/dist/` is build output, not hand-edited source.
- `docs/`: current cross-SDK documentation and an index of preserved history.
- `examples/`: runnable programs demonstrating both SDKs.
- Root `tests/`: cross-SDK conformance, documentation/layout checks and explicit
  shared public-API fixture support in `sdk_fixture.py`.
- Root README: project overview and language entry points, not another guide.
- Root manifests, lockfiles, licenses and tool configuration: repository/tool
  entry points. There is no project changelog duplicating package histories.

## Current code and build boundaries

```text
Python public exports                  TypeScript public exports
python/radiclepy/                       node/src/
        |                                  |
PyO3 adapter                           napi-rs adapter
python/native/src/                     node/native/src/
        |                                  |
        +---------- upstream radicle ------+
                       =0.25.1
```

Binding crates always live at `<language>/native/`. The root virtual workspace
contains `python/native` and `node/native`, uses resolver 3 and defaults to the
Python package for compatibility with root developer commands. CI selects
packages explicitly. Both adapters inherit the exact upstream constraint and
explicit `indexmap/std`, and share one root `Cargo.lock`.

The lock consolidation required an explicit dependency alignment: 31 shared
Python package selections moved to the existing newer Node baseline. No versions
were selected outside the original locks. See the
[dependency/feature evidence](archive/FINDINGS-WORKSPACE-LAYOUT.md); it is not a
claim that moving files alone was dependency-neutral.

Root `pyproject.toml` remains the Python Git-install entry point, points to
`python/native/Cargo.toml`, and uses the Python guide as its package description.
Python tests/typing/smokes live in `python/tests/`; pytest explicitly discovers
both test directories. Cross-SDK fixtures do not import Python-private tests.

Root `package.json` remains the npm Git-install entry point. Git `prepare` builds
optimized native output and compiles the wrapper in a complete checkout before
packing. The npm payload contains built output, language docs and licenses, not
Rust sources/manifests or an installed consumer rebuild contract. There are no
`install`/`postinstall` native build scripts. Neither package has been published.

Python source distributions must build without the checkout. Maturin rewrites
the packaged workspace to the Python member; normal offline Cargo may prune
unused Node lock records but must not select new versions. Strict immutable
sdist-lock support is not promised. Git/workspace Rust gates use `--locked`.
See [development](development.md) for the independent artifact checks.

Node's calling-thread setup, worker tasks and lock registry remain binding
implementation details. Python retains its own thread-affine handle contract;
see [SDK differences](sdk-differences.md).

## Shared semantics, not identical interfaces

[Workflow semantics](semantics.md) owns common behavioral rules. Language API
references own signatures, errors, conversions, snapshot representation and
concurrency. Keep executable guide examples and digest parity tests as evidence;
shared prose or matching method names alone do not establish conformance.

Upstream owns identity/COB evaluation, signing and Git operations. The SDKs should
remain thin adapters. They must not silently add networking, Git commit import,
rollback, automatic retry or a generalized write scheduler.

Source sharing does not imply shared ABI, shared runtime state or synchronization
across Python, Node, external processes or separately loaded addon copies.

## Structural migration status

Radicle issue `edcbbd7` tracks the migration. Language-owned docs, workspace/test
layout and module alignment are implemented. Both bindings own `profile`,
`project`, `repository`, `issues`, `patches`, `error` and `ids` modules; module
entry points register/export the bindings rather than implementing workflows.
Snapshots live near their domains, with a small collaboration comment type in
each adapter. Python's root `values.rs` now contains only process-local hashing,
not another shared DTO schema. Node tasks, repository locks, Date conversion and
native probes remain explicitly binding-specific. Python-only policy, routing
and raw access remain independent. [Development](development.md#module-alignment-map)
records the mapping. No shared Rust extraction is included in these moves.

## Shared Rust: only with a real consumer

There is no `core/` crate yet. Create it separately when both adapters call its
first real operation: bounded history traversal with checked skipping and
lookahead. Preserve native argument domains and error mapping in each adapter.

A private common crate should hold selected SDK-owned policy, not wrap every
upstream operation or define a third full DTO schema. Project-init preflight is a
later candidate. Preserve raw bytes and timestamp units in any common values.
Keep FFI attributes, Date-safe conversion, Python hashing, worker scheduling,
mutex ownership and panic guards in the language bindings. Do not impose Node
Send/Sync bounds on Python signers or introduce custom environment/agent adapters.

## Historical evidence

[Archived plans and findings](README.md#historical-plans-reviews-and-findings)
record earlier decisions, test counts and support evidence. They are not current
release checklists. The obsolete `spike/` and `wheel-probe/` source was removed
in a distinct cleanup commit. [Probe retrieval](archive/PROBES.md) records the
exact last-containing commit and commands to inspect or recover both directories;
they are not maintained workspace members.
