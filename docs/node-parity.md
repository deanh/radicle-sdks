# TypeScript capability parity with Python

Status: patch 1 merged at `d7b2ffcd4868fe93f9d89779ea517a27136f7f48`;
patch 2 implemented on `node-parity-policy`, review pending.
Checked items for patch 2 mean implemented, not yet approved/merged.
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
- [x] Home construction/loading/resolution and path access; profile home access.
  Keep these local setup operations distinct from a profile environment adapter.
- [x] Explicit storage opening and storage path access. `profile.storage` already
  provides the capability of Python `Storage.from_profile`; no redundant factory
  is required solely to match spelling.
- [x] Alias lookup and reverse lookup with equivalent grouped results and missing
  behavior; signer DID access.
- [x] Public `parseRepoId`, `parseDid`, `parseNodeId`, `parseOid`, `parseObjectId`:
  reuse upstream parsers/canonicalization, existing Node argument error codes,
  and synchronous panic containment. Document string ID types, not nominal
  runtime validation through TypeScript annotations.
- [x] Typed issue/patch store RID access, count and emptiness. Match upstream
  operations and error behavior rather than silently replacing count with an
  evaluated `all().length`.
- [x] Issue comment count/IDs and patch revision count: retain Python's actual
  counting semantics, including redaction cases, whether exposed as snapshot
  fields or documented equivalent access. Do not assume all counts equal array
  lengths without checking the implementation.
- [x] Audit all existing shared snapshot fields and option domains, not just
  the additions above; record any further gap here before declaring completion.

Required evidence: same-storage Git/identity/tag/ref fixtures, missing and invalid
inputs, readonly versus write effects, concurrency/failure recovery for new
writes, bare/non-bare destinations, private construction and public type checks.

### Patch 1 public-surface audit

Audited the baseline Python stubs against public Node declarations and both
implementations (including field materialization, not only method names):

| Python surface | Node equivalent / disposition |
| --- | --- |
| Five `parse_*` functions | Five synchronous camelCase `parse*` functions |
| `RepoId/Did/NodeId/Oid/ObjectId/Alias` string aliases | Ordinary string domains; no Python runtime `str` alias objects or new nominal types |
| `Home` constructor/load/resolve/path | Async `Home.create/load/resolve`; synchronous `path` |
| `Profile` init/load/did/node_id/home/storage/aliases/signer | Equivalent camelCase members; existing synchronous load/signer and async init preserved |
| Profile policy/routing conveniences | Implemented in patch 2 below |
| `Aliases.get/reverse_lookup` | Async `get/reverseLookup`, frozen grouped results; own-property preservation for `__proto__` |
| `Signer.did/node_id` | Synchronous `did/nodeId` |
| Storage profile factory/open/path/repository/contains/repositories | `profile.storage`, async `Storage.open`, path and existing operations |
| All repository reads and four writes | Equivalent camelCase methods; description from verified identity |
| `HeadInfo`, Commit, Tree/TreeEntry, TagInfo, RemoteInfo | All data fields mapped; bytes/timestamps/optional kind follow established Node conventions |
| ProjectDoc and HistoryPage | All fields mapped; camelCase and existing pagination contract retained |
| SetHead and SignedRefs | All fields; `isUpdated` boolean replaces `is_updated()`; readonly `refs` object replaces `refs()` dict |
| Issues/Patches open/open_as/rid/all/get/count/is_empty/create/get_mut, patch draft | Equivalent store methods, with `openAs/getMut/isEmpty` and options objects |
| Issue/Patch snapshots | All fields, including comment count/IDs and revision count; absent states remain Node optional fields |
| IssueMut ID/reload/comment/edit/edit_description/label/assign/close/reopen/lifecycle | All capabilities; close/reopen replace generic lifecycle dispatch; envelopes instead of cached `.issue` |
| PatchMut ID/reload/edit/label/assign/update/ready/unready/archive/unarchive/comment/review/merge | All capabilities; envelopes instead of cached `.patch` |
| Comment/Review/Revision | All fields; timestamps use the existing Date-safe nullable milliseconds convention |
| PolicyStore/RoutingStore and their values | Implemented in patch 2 below |
| Raw/CobRef/CobManifest/RawOp | Patch 3, still unchecked below |
| Python exceptions/protocols/repr/hash/equality | Explicit language differences below; not missing workflows |

Comment IDs/counts use `issue.comments()` in both implementations, excluding
redacted slots and including the root; patch revision count uses
`patch.revisions().count()` in both, excluding redacted slots but including other
authors. Existing `version` still counts all revision slots minus one and is not
an array index. Counts use upstream count/emptiness, not extra binding evaluation
or filtering. No unassigned patch-1 gaps remain from this inventory.

## Patch 2: policy and routing

- [x] Policy writable profile factory, read-only profile factory, explicit-path
  opening and path access; profile convenience access or documented equivalent.
- [x] `follow`, `unfollow`, `seed`, `unseed`, `setFollowPolicy`, `setSeedPolicy`,
  `unblockRid`, `unblockNid`.
- [x] `isFollowing`, `isSeeding`, `isBlocked`, `followPolicy`, `seedPolicy`,
  `followPolicies`, `seedPolicies`, `nodesByAlias`.
- [x] `FollowPolicyInfo` and `SeedPolicyInfo` fields; allow/block, scope, alias
  and absent-value distinctions, including blocked versus simply not followed.
- [x] Routing profile factory and equivalent profile convenience; `get`,
  `getInventory`, `entry`, `entries`, `count`, `total`, `isEmpty`.
- [x] `RoutingEntry` fields, ordering, missing timestamps and numeric limits.
- [x] Database/policy/read-only errors mapped to documented Node codes.

Connection ownership decision: policy handles retain only the database path and
access mode, not an open SQLite connection. Each asynchronous worker operation
opens, uses and drops its own upstream policy store, mirroring repository
reopening. A read-only handle must use upstream `Store::reader` (read-only open
flags), never the writable opener; opening per call alone does not prevent file
creation. No connection is transferred between jobs. Accept one open per call;
retain the no-retry contract and do not claim cross-process coordination.
Routing uses upstream `node::Database`'s shared thread-safe read-only connection,
with Send/Sync assertions and worker/concurrent-read tests.

Factories eagerly open/validate on a worker, then drop policy connections and
canonicalize the file path. Later reads always reopen read-only, including on
writable handles; a permitted write may recreate a deleted file. File-backed
policy stores intentionally reject empty/`:memory:` connection names and do not
interpret SQLite URIs. This is a consequence of the agreed per-operation design,
not an unnoticed promise of Python's held-connection ephemeral-store behavior.
Policy `.path` is canonical absolute rather than Python's explicit-path spelling.

No SDK policy mutex, queue or application retry is added: same-addon workers and
processes rely on SQLite's own locking and upstream read/write busy timeouts
(3/6 seconds). SQLite errors propagate as POLICY, routing errors as DATABASE;
read-only writes reject READ_ONLY after argument validation. Database query
panics from pinned unchecked upstream row decoding are contained as INTERNAL.
Do not mix distinct SQLite builds concurrently inside one process: per-library
VFS lock registries are independent. An external-process exclusive-lock test
checks busy failure and recovery; independent-handle/worker tests check commits.
Read-only factories are tested not to create or repair a missing/empty database.

Routing timestamp decision: use the existing Node millisecond/Date-safe conversion
with `null` for unrepresentable stored values. Record this as an intentional
representation difference from Python's arbitrary-precision integers; preserve
the established missing-value behavior.

Public-surface audit: all 16 policy read/write operations, three factories and
path access are mapped to Node, along with both policy value types. All seven
routing queries, its profile factory and RoutingEntry fields are mapped; both
Profile conveniences are implemented. Tuple pairs become frozen readonly arrays;
absent alias/scope use Node optional fields, absent lookups use null. No inventory
write, config-combining facade or extra scheduler was added. Connection/path and
timestamp representation differences are explicit above.

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
Use standard JSON.parse reviver source text rather than a custom scanner; the
source facility was locally verified on both Node 26.5.0 and 26.12.0. Source text
is provided for primitive tokens only, not object/array reviver visits; the
helper must check this and never infer a composite value from `context.source`.
The helper itself remains patch-3 work and still needs numeric boundary tests.

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
- File-backed Node policy connections reopened per operation, canonical `.path`,
  and no ephemeral `:memory:`/empty connection names; Python holds its connection.
- Non-UTF-8 stored path getters use Node INVALID_DATA instead of Python ValueError;
  routing timestamps outside the Date range use Node null instead of large integers.

New representation differences must be recorded explicitly; this list is not a
blanket exemption for missing data or unsupported workflows.

## Progress evidence

Patch 1 was reviewed and fast-forward merged unchanged at `d7b2ffc`: repository
inspection/metadata, canonical-ref writes, bare checkout, ID validators,
home/profile/storage utilities, aliases, typed-store conveniences and its audit.
The validation below records that patch's results, not a current cumulative count.

Local Linux x86_64 validation: fresh optimized Rust 1.91 addon build; 36 Node tests
on both Node 26.5.0 and 26.12.0; 209 Python/shared tests with helpers and cross-SDK
checks required; 5 native release tests; MSRV/stable Clippy and rustfmt;
TypeScript and Prettier checks. The packed-package gates on npm 12/Node 26.5 and
npm 11/Node 26.12 execute new parity runtime and type consumers outside the
checkout in addition to the existing runtime/actual README gates, without Rust
rebuilding. Lockfiles are unchanged. Exact committed-source installation evidence
and submitted/reviewed heads are recorded in the tracking issue. Remote CI,
macOS, manylinux and additional platform support are not established by these
local results. Evidence: `/tmp/radicle-node-parity-p1/` on the development host.

Patch 2 implements policy/routing and carries all seven patch-1 review nits:
upstream DID/SetHead rendering/decision helpers, INVALID_DATA for stored non-UTF-8
paths (both native getters tested), member/import ordering, count-conversion
comments and primitive-only reviver source notes. Local optimized Rust 1.91 build,
211 required-helper/cross-SDK Python tests, 44 Node tests on Node 26.5/26.12,
6 native release tests, MSRV/stable Clippy/fmt, tsc/Prettier and npm 11/12 packed
runtime/actual-guide/parity/type gates pass. Exact committed-source installations
and review heads are recorded in the tracking issue after commit. Lockfiles remain
unchanged. Evidence: `/tmp/radicle-node-parity-p2/`. No remote CI/platform expansion
claim and no raw COB implementation; patch 3 remains separate.

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
