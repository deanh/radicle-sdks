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

- `python/`: Python package/stubs, guide, API reference and changelog.
- `node/`: TypeScript wrapper, native binding, tests, scripts, guide, API reference
  and changelog. Generated `node/dist/` is build output, not hand-edited source.
- `docs/`: current cross-SDK documentation and an index of preserved history.
- `examples/`: runnable programs demonstrating both SDKs.
- Root `tests/`: intended home for cross-SDK conformance and documentation tests.
- Root README: project overview and language entry points, not another guide.
- Root manifests, lockfiles, licenses and tool configuration: repository/tool
  entry points. There is no project changelog duplicating package histories.

## Current code and build boundaries

This documentation step establishes language-owned guides. The Rust/test layout
has **not yet migrated**:

| Concern | Current location | Planned location |
| --- | --- | --- |
| Python Rust crate | root `Cargo.toml`, `src/` | `python/native/` |
| Python package and stubs | `python/radiclepy/` | unchanged |
| Python-only tests/typing/smokes | root `tests/` | `python/tests/` |
| Cross-SDK digest and docs tests | root `tests/` | unchanged |
| Node Rust crate | `node/native/` | unchanged |
| TypeScript wrapper/tests/scripts | `node/src/`, `node/test/`, `node/scripts/` | unchanged |
| Rust dependency graphs | root and `node/native/` locks | virtual workspace, one root lock |

The following is the **current** runtime layering:

```text
Python public exports                  TypeScript public exports
python/radiclepy/                       node/src/
        |                                  |
PyO3 adapter                           napi-rs adapter
src/                                   node/native/src/
        |                                  |
        +---------- upstream radicle ------+
                       =0.25.1
```

Root `pyproject.toml` is the Python Git-install/build entry point and selects the
Python guide as its package description. Root `package.json` is the npm
Git-install entry point; `prepare` builds optimized native output and compiles the
wrapper before packing. Neither package has been published to a registry.
Node's calling-thread setup, worker tasks and lock registry remain binding
implementation details; Python's thread-affine handles retain their own contract.
See [SDK differences](sdk-differences.md).

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

## Planned structural migration

Tracked in Radicle issue `edcbbd7`, with review refinements recorded there:

1. Language-owned guides/references/changelogs and current shared documentation.
2. Python native/test move and a virtual workspace, after proving Python sdist
   feasibility in an isolated experiment. Rename workflows by language and handle
   obsolete probes in a distinct cleanup commit.
3. Matching domain modules in both bindings: `profile`, `project`, `repository`,
   `issues`, `patches`, `error`; keep binding-specific modules explicit.

Binding crates then live at `<language>/native/`. Root manifests remain for Git
installation. The workspace should use resolver 3, deliberate default members,
explicit package selection, one lockfile and inherited exact Radicle constraints.
Dependency versions/features must be compared per package, not casually refreshed.
If workspace packaging proves impractical, a separately reviewed fallback keeps
Node standalone, sacrificing the single lock and inherited constraints for it.

[Development](development.md#architecture-migration-gates) records validation and
module mapping. This plan is not a claim that the later steps have already landed.

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
release checklists. `spike/` and `wheel-probe/` remain historical source until the
layout step checks references and records exact retrieval commits before removal.
They must not become maintained workspace members merely through ancestry.
