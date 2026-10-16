# Binding module alignment findings

Architecture issue `edcbbd7`, structural patch 3; baseline
`63b90b4efe6f995f6d4af655bf8b2574975c52f5`. This is an operation/snapshot ownership
move, not API normalization, shared Rust extraction or a dependency refresh.
Both SDK versions remain `0.1.0`.

## Ownership

Both bindings now name `profile`, `project`, `repository`, `issues`, `patches`,
`error` and `ids` responsibilities explicitly. Their file trees need not be
identical. Python's module entry registers classes/functions in the original
order; Node's entry declares modules, re-exports native free functions and keeps
compile-time handle assertions. Neither entry implements workflows.

Python:

- `repository/`: storage/repository operations, write helpers and their values.
- `issues/`, `patches/`: store/handle operations in `mod.rs`; immutable values and
  materialization beside the domain in `values.rs`.
- Project, policy and routing snapshots live in their own domain directories.
- `comment.rs` owns the small shared collaboration snapshot/converter.
- `hashing.rs` is only the existing process-local randomized hash helper; all
  snapshots still use the same key, not one per domain.
- `ids.rs` contains public identifier helpers plus existing internal validators.
  The object-ID validator retains its distinct error wording from public parsers.
- `raw.rs` renames raw COB access; policy/routing/raw remain Python-specific.

Node:

- `profile.rs`, `project.rs`, `repository/`, `issues/`, `patches/`,
  `error.rs` and `ids.rs` own operations and parsers.
- Repository/issue/patch DTOs and materialization live in domain `values.rs`
  files beside operations in `mod.rs`. `comment.rs` owns the common DTO.
- `tasks.rs`, `locks.rs`, `timestamps.rs` and `probes.rs` explicitly own binding
  scheduling/containment, write-lock registry, Date conversion and native probes.
- Sibling handles need crate-visible storage/signer internals; no new JS getters
  or public wrapper constructors were added. Imports are explicit, not a service
  locator in `lib.rs`.

## Preservation checks

The root lock hash and both per-package feature trees are byte-identical to the
baseline. No manifests, public Python stubs, TypeScript wrapper sources or build
configuration changed. Unrelated formatting in untouched Python modules remains
untouched; assembled modules use normal formatting.

A local syntax-tree audit compared all non-registration Rust function/method
bodies with baseline: **367 Python and 90 Node** bodies match after normalizing
module paths, the internal object-ID helper name and formatting-only trailing
commas. The trivial Python `cob_err` forwarding shim becomes a direct import of
its existing target. Registration functions are intentionally excluded from that
audit and covered by export/API tests. This is a migration guard, not proof of
all FFI or upstream behavior. Transaction macros retain their original bodies.

Native JS export-name sets match exactly. Generated native TypeScript declaration
ASTs match after sorting declarations and ignoring comments; only declaration
order changes. Public package typing/constructor tests continue to pass.

## Local validation

- Rust 1.91 helper build: **203 Python tests**, required cross-SDK mode enabled.
- Fresh optimized Node build in a new target directory: **30 Node tests**, public
  consumer typing and **4 native release tests**.
- Per-package Clippy with warnings denied on Rust 1.91 and stable; Python checked
  with and without test helpers. Node fmt and abort-mode rejection pass; the
  expected unwind requirement is retained.
- Fresh Rust 1.91 abi3 Python release wheel, installed on CPython 3.9 and 3.14:
  each **90 passed / 113 helper-dependent skips**, production/workflow purity and
  metadata smoke, stubtest, strict mypy, pyright and **100%** completeness
  (337 known public symbols). Installed completeness runs outside the checkout
  with the consumer interpreter. The actual guide fences run through exports.
- Packed Node consumers pass under pinned npm 11.20.0 and 12.2.0, including actual
  installed README/workflows/types with Rust tools unavailable during install.
- The clean unpacked sdist includes all nested Rust modules and installs via pip
  offline using cached maturin/Cargo dependencies on Rust 1.91. Cargo prunes 19
  unused Node lock records and adds none; installed production workflow smoke
  passes. This is source closure, not an immutable-lock/vendoring claim.

Exact-commit installation evidence is recorded in patch/issue discussion after
committing the candidate. These are local Linux x86_64 GNU/glibc checks, not
remote CI, macOS or manylinux_2_28 evidence. Local wheels are manylinux_2_39;
there is no registry publication, portable prebuilt promise or `core/` crate.
