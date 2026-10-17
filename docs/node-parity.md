# TypeScript capability parity with Python

Status: patch 1 in progress on `node-parity`; no parity patch merged yet.
Checked capability items mean implemented on this branch, not reviewed/released.
Tracking issue: `4047bd966f4e39b98d097ff10d34dad649e37195`.
Baseline: `4aa27eb10d1e6a4598629702086e9c35571340ef`.

## Completion contract

Make every supported Python capability available to TypeScript consumers, while
keeping the existing Node execution, error and snapshot conventions. The Python
public stubs (`python/radiclepy/__init__.pyi`), implementation and API reference at
the baseline are the inventory, not the entire upstream Radicle API.

Deliver three independently reviewable implementation patches, each including
its own tests, API documentation, changelog and installed-consumer evidence.
The final review reconciles this checklist with both public export surfaces; it
is not a fourth feature patch. Review approval does not authorize merging.

Keep `radicle = "=0.25.1"`, Rust >=1.91, Node >=26.5.0 and both package versions
at 0.1.0. No incidental dependency refresh. No networking, node management,
working-copy content commit creation/import/push, scheduler, registry release,
platform expansion or shared-core extraction is required by this work.

## Already available: verify, do not reimplement

- Profile initialization/loading, signer access and identifiers; storage lookup,
  existence and enumeration; local project init/fork/non-bare checkout.
- Repository head/identity, branches/branch head, commit/tree/blob, bounded
  history and signed refs.
- Issue reading/creation, comments, title/description, labels/assignees and
  lifecycle; patch reading/creation/drafts, revisions, comments, reviews,
  labels/assignees, lifecycle and merge records.
- Comment/review/revision snapshots and existing Git byte/metadata access.

These capabilities still participate in the final field-by-field audit and
regression suite. Presence of a method alone does not establish parity.

## Patch 1: repository and utilities

- [x] Repository `description`, `tags`, `revParse`, `remotes`; `TagInfo` and
  `RemoteInfo` fields equivalent to Python, with deterministic ordering and
  annotated/lightweight/non-commit tag cases.
- [x] Identity snapshot fields currently missing from `ProjectDoc`: `version`,
  `threshold`, `commit`, `blob`.
- [x] `setDefaultBranchToCanonicalHead`, `setIdentityHead`,
  `setRemoteIdentityRoot`, including `SetHead` old/new/change information.
  Preserve upstream no-op/idempotence and persisted side effects; use existing
  repository write locking, without acquiring a signer for unsigned operations.
- [x] Bare checkout option without changing existing non-bare calls.
- [ ] Home construction/loading/resolution and path access; profile home access.
  Keep these local setup operations distinct from a profile environment adapter.
- [ ] Explicit storage opening and storage path access. `profile.storage` already
  provides the capability of Python `Storage.from_profile`; no redundant factory
  is required solely to match spelling.
- [ ] Alias lookup and reverse lookup with equivalent grouped results and missing
  behavior; signer DID access.
- [x] Public `parseRepoId`, `parseDid`, `parseNodeId`, `parseOid`, `parseObjectId`:
  reuse upstream parsers/canonicalization, existing Node argument error codes,
  and synchronous panic containment. Document string ID types, not nominal
  runtime validation through TypeScript annotations.
- [ ] Typed issue/patch store RID access, count and emptiness. Match upstream
  operations and error behavior rather than silently replacing count with an
  evaluated `all().length`.
- [ ] Issue comment count/IDs and patch revision count: retain Python's actual
  counting semantics, including redaction cases, whether exposed as snapshot
  fields or documented equivalent access. Do not assume all counts equal array
  lengths without checking the implementation.
- [ ] Audit all existing shared snapshot fields and option domains, not just
  the additions above; record any further gap here before declaring completion.

Required evidence: same-storage Git/identity/tag/ref fixtures, missing and invalid
inputs, readonly versus write effects, concurrency/failure recovery for new
writes, bare/non-bare destinations, private construction and public type checks.

## Patch 2: policy and routing

- [ ] Policy writable profile factory, read-only profile factory, explicit-path
  opening and path access; profile convenience access or documented equivalent.
- [ ] `follow`, `unfollow`, `seed`, `unseed`, `setFollowPolicy`, `setSeedPolicy`,
  `unblockRid`, `unblockNid`.
- [ ] `isFollowing`, `isSeeding`, `isBlocked`, `followPolicy`, `seedPolicy`,
  `followPolicies`, `seedPolicies`, `nodesByAlias`.
- [ ] `FollowPolicyInfo` and `SeedPolicyInfo` fields; allow/block, scope, alias
  and absent-value distinctions, including blocked versus simply not followed.
- [ ] Routing profile factory and equivalent profile convenience; `get`,
  `getInventory`, `entry`, `entries`, `count`, `total`, `isEmpty`.
- [ ] `RoutingEntry` fields, ordering, missing timestamps and numeric limits.
- [ ] Database/policy/read-only errors mapped to documented Node codes.

Connection ownership decision: policy handles retain only the database path and
access mode, not an open SQLite connection. Each asynchronous worker operation
opens, uses and drops its own upstream policy store, mirroring repository
reopening. A read-only handle must use upstream `Store::reader` (read-only open
flags), never the writable opener; opening per call alone does not prevent file
creation. No connection is transferred between jobs. Accept one open per call;
retain the no-retry contract and do not claim cross-process coordination.
Routing can use upstream `node::Database`'s thread-safe shared connection, subject
to verifying the wrapper's worker ownership and error handling.

Before implementation, finish specifying factory-versus-operation error timing,
serialization scope and busy/error propagation. Per-operation connections do not
by themselves serialize concurrent writes. Tests must prove a missing read-only
database remains absent and that writable access is never obtained as fallback.

Routing timestamp decision: use the existing Node millisecond/Date-safe conversion
with `null` for unrepresentable stored values. Record this as an intentional
representation difference from Python's arbitrary-precision integers; preserve
the established missing-value behavior.

Required evidence: Python-to-Node and Node-to-Python policy visibility, read-only
failures, replacement/no-op booleans, missing/empty databases, concurrent handles
and workers, and routing fixtures that do not require a network daemon.

## Patch 3: raw COB inspection

- [ ] `Raw.open`, RID, `typenames`, `entities`, `refs`, `history`.
- [ ] `CobRef` name/target; `CobManifest` type name/version.
- [ ] `RawOp` ID, actor, timestamp, action bytes, parents, related links, identity
  link and manifest, plus JSON decoding capability.
- [ ] Preserve strict ref/identifier acceptance, deterministic enumeration,
  causal-parent-only traversal, cross-ref deduplication, topological order and
  deterministic ties, and backend/decode/cycle error propagation.
- [ ] Independent action byte copies and immutable surrounding snapshots; no
  accidental claim that individual COB refs authenticate their contents.

Action JSON decision: retain independent action byte copies and provide a
no-dependency decoding helper that preserves integers outside JavaScript's safe
integer range as `bigint`, with safe integers represented as `number`. Determine
the integer from its original JSON token, never from an already-rounded Number.
A string-aware number scanner is an option; prefer the standard JSON.parse reviver
source text if verified on the minimum supported Node version. This facility was
locally verified on Node 26.12, not yet on the 26.5 floor.

Specify decimal/exponent behavior before implementing the helper: BigInt cannot
represent fractional values, so integer preservation must not be advertised as
arbitrary-decimal losslessness. Python itself decodes decimal/exponent tokens as
floats. Test positive/negative safe-integer boundaries, nested arrays/objects,
escaped strings containing number-like text, negative zero, fractional/exponent
tokens, invalid JSON and overflow behavior. Do not introduce a dependency or
silently round out-of-range integer tokens. Document that decoded BigInts cannot
be passed directly to ordinary JSON.stringify; original bytes remain available.

Raw timestamp representation remains to be specified. Python raw timestamps are
seconds, unlike typed COB milliseconds; the routing decision does not silently
apply to this low-level format.

Required evidence: shared histories covering multiple actors/refs, identity and
related links that must not be traversed, deterministic ordering, malformed refs,
invalid payloads, numeric boundaries and observable read-only behavior.

## Intentional language differences

Retain these rather than copying Python object mechanics:

- Async Node repository/project/COB work, existing synchronous profile/signer
  setup, lazy repository/store handles and private native construction.
- Node argument `TypeError` codes and operational `RadicleError` codes, rather
  than Python exception classes; equivalent failure distinctions remain required.
- Frozen snapshots, independent mutable byte copies, `null`/optional fields per
  the established Node contract, and camelCase/options-object APIs.
- Mutation result envelopes and explicit reload, not cached `.issue`/`.patch`
  properties; close/reopen already provide Python lifecycle capabilities.
- No Python structural hashing/equality, repr, membership/iteration protocols or
  PathLike emulation. Ordinary JS collections and string paths are sufficient.
- Existing typed timestamp conversions and documented runtime/platform boundaries.

New representation differences must be recorded explicitly; this list is not a
blanket exemption for missing data or unsupported workflows.

## Progress evidence

Patch 1 first implementation slice (not yet ready for whole-patch review):
repository inspection and metadata, canonical-ref writes, bare checkout and ID
validators. Profile/home/storage utilities, aliases and typed-store conveniences
remain unchecked above.

Local Linux x86_64 validation: optimized Rust 1.91 addon build; 34 Node tests;
208 Python/shared tests with helpers and cross-SDK checks required; 4 native
release tests; MSRV/stable Clippy and rustfmt; TypeScript and Prettier checks.
The packed-package gate now runs the new parity consumer and repository type
fixture outside the checkout, in addition to the existing runtime/README gates.
No exact-SHA Git installation, minimum-Node runtime or remote CI claim for this
slice. Evidence: `/tmp/radicle-node-parity-p1/` on the development host.

## Completion review and evidence

- [ ] Every Python public export/member is mapped to a Node capability or a
  specific intentional difference; include convenience APIs and nested fields.
- [ ] New APIs are exported only through the installed package's public entry;
  runtime and compile-time consumer tests verify that contract.
- [ ] Cross-SDK fixtures compare persisted state and normalized results, with
  tests for validation order, absence, malformed data and failure effects.
- [ ] Each patch has fresh optimized builds, required-helper Python and Node
  suites, native tests, MSRV/stable lint/format, typing, actual guide execution,
  packed npm consumers and exact-SHA Git installation evidence as applicable.
- [ ] Final cumulative checks show no Python regressions, no dependency drift,
  and no unreviewed convenience/field gaps. Record local evidence separately
  from remote CI or untested platform claims.
- [ ] Update `docs/sdk-differences.md`, language references/guides/changelogs and
  scope statements so completed additions are no longer described as deferred.
- [ ] Record reviewed heads and evidence in the tracking issue and close it only
  when all three implementation patches are merged and the final audit passes.
