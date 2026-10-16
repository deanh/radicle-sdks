# TypeScript SDK proposal review

Reviewed against `TS-SDK-PROPOSAL.md` and the current Python SDK after merging
its Pythonic UX patch (`11b2e60`). This is review feedback, not an implementation
plan or authorization to extract a shared core.

## Overall assessment

The direction is sound: napi-rs is a sensible default, and the three local
workflows provide a useful, bounded finish line. Approve a bounded T0 spike;
make the shared-core decision after its results rather than treating extraction
as inevitable.

The user is not concerned about requiring a named first consumer. If one is
useful for scope and runtime decisions, it is **rad-pi, the pi extension for
Radicle**. Naming or validating a consumer is **not a prerequisite to T0**.
Use rad-pi as a reference when choosing scope, not as a separate demand-validation
project or justification for adding features.

## Findings

### 1. Medium — make core extraction conditional; update stale coordination notes

The proposal moves from an experimental second binding to assuming a shared-core
extraction is justified. A second binding makes sharing worth evaluating, but
does not establish which abstractions should be shared before the spike.

Revise T1 to depend on a successful T0 and an explicit architecture decision:
what reusable operations were demonstrated, what extraction costs Python users,
and whether that cost is worthwhile. Do not add a consumer-confirmation gate;
the user's stated interest and rad-pi are sufficient context to proceed.

Remove the stale references to another agent implementing Python UX Patch 1.
Python UX patches 1, 2 and 5, plus supporting tooling, are already merged.

### 2. Medium — specify the concurrency contract more precisely

A per-RID mutex is a reasonable conservative starting point, but its scope and
coverage need to be explicit:

- Independently opened handles for the same underlying repository must share
  serialization. Specify repository identity, including the storage location;
  handle-local locks are insufficient.
- Cover the complete operation: reload, transaction, signing and publication of
  the refreshed snapshot. Do not read mutable transaction state before acquiring
  the lock or publish stale cached state from out-of-order Promise callbacks.
- Define a consistent ordering for repository and signer locks.
- State that binding-side locks do not coordinate external Git commands, other
  processes or other implementations.

The Python implementation reloads the entry inside each transaction in
`src/cob_typed.rs`; preserve that property.

T0 should test concurrent operations through both the same mutable handle and
separate handles, including failure recovery, persisted results and snapshot
freshness. Preserve commit-per-operation behavior.

Also, mutex waiters occupy libuv worker threads. A pool filled with waiting
writes can delay reads and unrelated pool work. Therefore, "reads stay parallel"
is not an unconditional guarantee. Measure contention in T0 or soften that
claim. Do not introduce a custom scheduler before evidence requires one.

### 3. Medium — define timestamp overflow and precision behavior

The milliseconds-as-number convention is reasonable, but ordinary timestamps
are not enough to validate it.

Require checked conversion from upstream seconds to milliseconds and an explicit
policy for integers outside JavaScript's safe-integer range. Multiplication can
overflow before conversion, and floating-point conversion can lose precision.
Peer-controlled metadata must not silently wrap or round.

If every exposed timestamp is advertised as directly usable with `Date`, also
distinguish its narrower valid range from `Number.MAX_SAFE_INTEGER`.

T0 should test boundaries and invalid metadata, as well as confirming the napi-rs
mapping produces the intended JS values and generated declarations. Define the
error behavior without introducing BigInt into the public API by accident.

### 4. Medium — generated types do not establish runtime behavior

Generated declarations reduce drift; they do not eliminate behavioral checks.
T0 should verify the actual exported contract:

- Finite literal unions rather than unrestricted strings.
- The intended readonly declarations.
- The selected byte representation and ownership/copy behavior.
- Promise signatures for I/O operations.
- Actual `RadicleError` instances, stable codes and useful messages, including
  errors rejected from worker tasks.
- The intended `TypeError`/`RangeError` distinction for invalid arguments.

TypeScript `readonly` does not freeze objects at runtime. Either document
mutable, detached snapshots as an intentional difference from Python, or
explicitly freeze them. Eager copies and runtime immutability are different
properties.

Also distinguish synchronous, metadata-only handle construction from operations
that open or read storage. The proposed chaining can work with lazy handles, but
I/O-bearing properties such as repository head reads need a clear asynchronous
API shape.

### 5. Medium — strengthen extraction and distribution gates

Keeping `pyproject.toml` at the repository root is necessary but does not alone
prove Git-install compatibility after moving the Rust package.

T1 should preserve and validate:

- Installation from an exact Git commit at the repository root.
- Self-contained sdist installation, including the shared core's sources.
- abi3 and Python 3.9 compatibility.
- Installed exports, stubs, package/runtime versions and release-helper exclusion.
- Existing Python API and behavioral contracts, not merely source-tree tests.

A workspace lockfile does not enforce a single upstream version: Cargo can lock
multiple versions simultaneously. Use a shared exact workspace dependency
(`radicle = "=0.25.1"`) with member inheritance, or make the shared core the sole
owner of that dependency where practical.

Independent tag prefixes require deliberate CI changes. The current workflow
triggers on `v*`, and `tests/wheel_smoke.py` expects `v{package_version}`.
Path-filtered CI must also run both bindings when shared-core, workspace or
relevant packaging inputs change.

The source-install story belongs in T0. Verify installation as a Git dependency
from the actual Node package location, with required Rust sources available and
build-script permissions documented. A local addon build alone is insufficient.
Test the package managers actually claimed as supported; keep Bun best effort
unless the spike establishes more.

### 6. Low — align T0 work with its exit criteria

T0 lists init and issue creation/commenting, but its exit criterion says Workflow
1, which also includes inspect/fork/checkout.

Either add those operations to T0 or narrow the criterion to match what the spike
actually demonstrates. Record reproducible pinned-version evidence for the
Send/Sync assertions, concurrency findings, generated types and installation
results rather than referring only to an earlier session's compile check.

## Recommended next step

Proceed with **T0 → findings → explicit architecture decision**. rad-pi can guide
choices without blocking the spike on consumer validation.

Defer broad shared-core extraction and a language-neutral conformance framework
until the spike shows what is useful to share. Initially reuse the three workflow
expectations with small language-specific tests; add a scenario framework only
if maintaining those tests demonstrates a need.
