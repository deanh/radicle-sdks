# radicle-sdk: TypeScript API contract

The current `0.1.0` alpha contract for Node.js/TypeScript, pinned to upstream
Radicle `=0.25.1`. The full reviewed Git SHA identifies a candidate. See the
[guide](README.md) for installation and an executable quick start, and the
[changelog](CHANGELOG.md) for package history. Only `radicle-sdk` is exported;
native bridge and probe files are internal, not supported import paths.

## Implemented surface

- Async `Profile.init`; synchronous `Profile.load()`/`signer()` setup and profile
  metadata/storage handles.
- Lazy `Raw.open(repo)` with async generic COB enumeration/refs/causal history;
  exact raw numeric metadata and synchronous `RawOp.actionsJson()` decoding.
- Async policy factories/reads/writes and read-only routing inspection; profile
  conveniences `policyStore()` and `routingStore()`.
- Async `Home.create/load/resolve`, `profile.home`, `profile.aliases`, storage
  path access and explicit async `Storage.open(path, alias, nodeId)`.
- Lazy `storage.repository(rid)`; async storage `contains/repositories`.
- Async repository `head/identity/description/branches/branchHead/tags/revParse`,
  `commit/tree/blob/history/remotes`, explicit `signRefs(signer)` and canonical-ref
  maintenance (`setDefaultBranchToCanonicalHead/setIdentityHead/setRemoteIdentityRoot`).
- Synchronous `parseRepoId/parseDid/parseNodeId/parseOid/parseObjectId` validators.
- Async `initProject` (public by default, or `visibility: 'private'`),
  `forkProject`, `checkoutProject` (non-bare by default, optional `{ bare: true }`).
- Lazy `Issues.open/openAs`; store `rid`, async `all/get/count/isEmpty/create/getMut`.
- Mutable issue handle with `id`, async `reload/comment/edit/editDescription`,
  `label/assign/close/reopen`.
- Lazy `Patches.open/openAs`; store `rid`, async `all/get/count/isEmpty/create/draft/getMut`.
- Mutable patch handle with `id`, async `reload/edit/label/assign/update`,
  `ready/unready/archive/unarchive`, revision `comment/review`, and merge records.
- Generated readonly DTOs, literal state/visibility domains and copied byte fields.
- Real `RadicleError` instances/codes and argument-error translation.

Generated declarations describe exact consumer types; the behavioral guarantees
and limits below supplement those types rather than promising upstream parity.

## Working-copy project discovery

`projectAt(path, { remoteName?: string } = {}) -> Promise<ProjectLocation | null>`
runs explicit-path Git discovery and named fetch-remote inspection on the native
worker pool. It searches parents and linked worktrees; it never changes cwd or
uses a Git environment repository override. No discovered repository or selected
remote returns `null`. Missing input paths, Git/configuration failures and invalid
selected URLs reject `GIT`; invalid remote names reject `ERR_INVALID_ARG_VALUE`
and wrong input types reject `ERR_INVALID_ARG_TYPE`.

The frozen readonly `ProjectLocation` has `rid: string`, `remoteName: string`, and
optional `namespace: string`. Remote name defaults to `rad`; another name must be
requested explicitly, never guessed. Only the fetch URL is parsed using upstream
`git::Url` (`rad://<canonical-rid>[/<node-id>]`). No profile, signer, storage lookup,
Git executable, network or writes are required. The result is unverified config
metadata, not an identity verification or storage `Repository` handle. Bare Git
repositories can also be inspected. Jujutsu discovery is excluded. Upstream
`rad::at`/`rad::remote` hard-code `rad`; this adapter adds explicit remote selection
using the upstream Git/URL APIs rather than emulating upstream cwd detection.

## Handle and snapshot contract

Handles have private constructors in the emitted consumer declarations and reject
direct JavaScript construction. Use `Home.create/load/resolve`, `Profile.init/load`, `profile.signer()`,
`profile.storage`, `storage.repository()`, `Issues.open/openAs` and
`Patches.open/openAs`; obtain mutable handles from `create/draft/getMut`.
No native conversion constructors or public `fromNative` helpers are exposed.

`initProject(storage, signer, path, { name, description, defaultBranch, visibility? })`
initializes an existing Git working copy with a committed default branch.
Visibility defaults to `'public'`; `'private'` uses upstream private visibility
with an empty allowlist. This is identity/access metadata, **not encryption** or
a network publication operation. Other string values reject
`ERR_INVALID_ARG_VALUE`; wrong input types reject `ERR_INVALID_ARG_TYPE`.
An existing `rad` remote is preserved and reinitialization rejected. Initialization
has no general rollback guarantee.
`checkoutProject(storage, rid, remote, path, { bare?: boolean })` defaults to a
non-bare checkout; `{ bare: true }` requests upstream bare checkout. Existing
four-argument calls are unchanged. Wrong option types, including `bare: null`,
reject `ERR_INVALID_ARG_TYPE`; no truthiness coercion is performed.

`parseRepoId`, `parseDid`, `parseNodeId`, `parseOid` and `parseObjectId` validate
and return canonical strings using upstream parsers. They throw synchronously:
malformed strings produce `ERR_INVALID_ARG_VALUE`; non-strings produce
`ERR_INVALID_ARG_TYPE`. They perform no repository I/O. TypeScript string types
do not replace these runtime checks.

Snapshots/result envelopes recursively freeze ordinary objects/arrays. Byte
fields are independent, mutable `Uint8Array` copies; changing one cannot alter
storage or another snapshot. Handles cache no snapshots. A comment result pairs
its entry ID with the post-operation snapshot built under the write lock.
Missing `Issues.get`/`Patches.get` returns `null`; missing repositories fail at first I/O.
This alpha exposes the bounded surface above, not the full upstream crate.

### Home, profile utilities and explicit storage

`Home.create(path)` creates/canonicalizes the home and required subdirectories;
`Home.load(path)` validates an existing layout without creating it.
`Home.resolve()` uses the native process environment (`RAD_HOME`, otherwise
`HOME/.radicle` on Unix) and creates missing directories, just like upstream.
These factories are asynchronous and return private-construction `Home` handles
with a synchronous `path` getter. Failures reject `STORAGE`. `profile.home`
provides its existing home without re-resolving the environment.

`profile.aliases` is an async lookup facade, not an enumerable database:
`get(nodeId)` returns a string or `null`; `reverseLookup(alias)` returns a frozen
`Readonly<Record<string, ReadonlyArray<string>>>` with upstream grouped matches.
Keys such as `__proto__` are preserved as own data properties, not interpreted as
prototype setters. Missing matches yield `{}`; upstream alias lookup semantics,
including its handling of unavailable sources, are retained. Invalid IDs/aliases
reject `ERR_INVALID_ARG_VALUE`; non-string inputs reject `ERR_INVALID_ARG_TYPE`.
The profile and its storage are retained safely across worker jobs. `signer.did`
is synchronous public metadata, like `signer.nodeId`.

`Storage.open(path, alias, nodeId)` asynchronously opens/creates upstream local
storage with explicit Git user identity; most callers should use
`profile.storage`. Alias and node ID are validated before filesystem creation.
`storage.path` preserves the upstream path, which need not be absolute for explicit
opening. Opening does not validate the inventory: even an existing file path can
be accepted upstream and fail on the first discovery call. Operational opening
and discovery failures reject `STORAGE`; invalid inputs use argument errors.
String paths are required; no PathLike or native-handle constructors are exposed.
A stored home/storage path containing non-UTF-8 bytes cannot be represented as a
JS string; reading `.path` throws `RadicleError` with `INVALID_DATA` (not an
argument error).

### Storage and repository contract

- `storage.repositories()` eagerly returns a frozen array of RIDs in upstream
  storage order (not sorted). Storage failures reject `STORAGE`, never an empty
  success. `contains(rid)` returns a boolean, with malformed RIDs rejected as
  `ERR_INVALID_ARG_VALUE` and upstream failures as `REPOSITORY`. Repository handles
  stay lazy: an absent repository fails at first I/O as `REPOSITORY`.
- `branches()` returns sorted **canonical** short names under `refs/heads/`,
  not actor namespaced branches. `branchHead(shortName)` follows symbolic refs;
  only upstream `NotFound` becomes `null`. Names are validated but not normalized:
  `refs/heads/main` as input means the short name `refs/heads/main`, not `main`.
  Other Git failures reject `GIT`. Enumeration follows libgit2, including its
  handling of malformed loose refs; iterator errors are propagated.
- `identity()` includes project name/description/default branch, identity `version`,
  delegates/quorum `threshold`, visibility, identity `commit` and document `blob`
  OIDs. `description()` reads that verified identity; failures reject `IDENTITY`,
  not a missing description sentinel (repository-open failures remain `REPOSITORY`).
- `tags()` returns name-sorted `TagInfo` values: `name`, raw `target`,
  `kind: 'annotated' | 'lightweight'`, `peeled`, and
  `peeledKind: 'commit' | 'tree' | 'blob'`. Symbolic tag refs are skipped. Only tag
  objects are peeled; lightweight commit targets do not accidentally become trees.
  Git/peeling failures reject `GIT`. `revParse(rev)` resolves arbitrary Git revision
  expressions without implicit peeling and rejects invalid/unresolved revspecs as
  `GIT`, not argument-value errors.
- `isAncestor(ancestor, descendant)` returns reflexive plain-Git commit ancestry
  through upstream/libgit2, not COB causal ancestry or signature verification.
  Both OIDs must be loadable commits, including equality (true); tags are not
  implicitly peeled. Malformed OIDs reject `ERR_INVALID_ARG_VALUE`, wrong types
  reject `ERR_INVALID_ARG_TYPE`, and wrong-kind/missing endpoints or native graph
  errors reject `GIT`, never silently false. No writes, signer, whole-graph
  integrity-audit or constant-time guarantee.
- `namespaceTips()` returns a node-ID-sorted frozen array of frozen
  `NamespaceTip` values `{ nodeId, target }`, with resolved sigrefs OIDs. It reads
  refs only, not target objects or signed sets: missing objects and non-commit
  targets are listable. Symbolic refs are followed; malformed namespace IDs or
  unresolved refs reject `GIT`. No atomic multi-ref snapshot is promised.
- `signedRefsAt(nodeId, oid)` loads an explicit sigrefs commit through upstream's
  signature, repository-binding and replay checks, returning the existing frozen
  `SignedRefs` shape (including frozen `refs`). It neither consults nor modifies
  the current sigrefs ref and does not verify COB signatures or authorization.
  Invalid IDs reject `ERR_INVALID_ARG_VALUE`; object/format/verification failures
  reject `GIT` naming the requested OID, never an empty set. Upstream may return
  an ancestor in `at` for legacy replayed signatures and promotes verified root
  commits from feature level `root` to `parent`; keep the requested OID separately.
  Legacy reads may walk sigrefs history.
- `remotes()` returns fetcher-sorted `{ fetcher, heads }` snapshots of upstream
  signed ref names (not just branches). Git/remote decoding failures reject `GIT`.
- `setDefaultBranchToCanonicalHead()` returns `{ old?, new, isUpdated }`;
  `setIdentityHead()` and `setRemoteIdentityRoot(nodeId)` return OIDs. Each maps
  directly to one upstream write under the shared repository lock and takes no
  signer. They do not implicitly sign refs or compose a transaction with other
  calls. Remote-root writes do not require membership and are idempotent upstream.
  Invalid node IDs reject argument errors; operational failures reject
  `REPOSITORY`. There is no rollback/retry or external-writer coordination.
- `commit(oid)` includes tree/parent OIDs, author/committer name/email bytes and
  lossy `...Str` views, nullable epoch-millisecond times, timezone offsets in
  minutes, message bytes/lossy text, and optional declared encoding. `tree(oid)`
  preserves canonical Git entry order (directories sort as `name/`), raw filename
  bytes/lossy `nameStr`, optional finite object kind, raw Unix mode, and OID.
  Symlinks are blobs with mode `0o120000`; gitlinks use `0o160000`. No re-sorting,
  implicit object peeling, text decoding of blobs, or filesystem traversal.
- `blob(oid)` returns an independent mutable `Uint8Array`. Every byte field in
  commits/trees also owns its own copy, while DTOs/ordinary arrays remain frozen.
  Missing/wrong-kind objects reject `GIT`; malformed OIDs reject argument errors.
- `history(rev, { limit = 100, offset = 0 })` resolves a Git revision expression
  and returns `{ name, head, oids, offset, hasMore }`. `name` preserves the input;
  OIDs use libgit2's default revwalk order, **not** a promised chronological or
  topological sort. It reads at most `limit + 1` entries after skipping offset.
  Pagination requires finite safe JS integers, `limit >= 1`, `offset >= 0`;
  larger valid limits clamp to 1000. Invalid values reject `ERR_INVALID_ARG_VALUE`,
  wrong input types reject `ERR_INVALID_ARG_TYPE`. Revspec/walk errors reject
  `GIT`, including errors encountered while skipping and a resolved non-commit
  object that cannot start the walk.
- `signRefs(signer)` directly calls upstream signing under the shared repository
  lock, then signer guard, returning a frozen `{ nodeId, at, parent?, featureLevel,
  refs }`. `featureLevel` is `none | root | parent`; `refs` is a readonly plain
  refname-to-OID object from the upstream result, not a later racing ref read.
  The structural `refs/rad/sigrefs-parent` blob entry is omitted upstream; use
  `parent` for that metadata. Upstream may reuse a sigrefs commit when unchanged.
  Signing does not import commits, move canonical heads, or sync. Use it after
  direct Git imports/ref changes; issue/patch COB writes already sign refs.
  Errors reject `REPOSITORY`; there is no rollback/retry guarantee.
- Reads remain parallel; signing and canonical-ref maintenance share the write lock with issue/patch writes
  across handles/workers using the same addon. External Git/CLI writes are not
  coordinated: finish them before asking the SDK to sign/read their results.

```ts
const commit = await repo.commit((await repo.head()).oid);
const tree = await repo.tree(commit.tree);
for (const entry of tree.entries) {
  if (entry.kind === 'blob') console.log(entry.nameStr, await repo.blob(entry.oid));
}
const page = await repo.history('HEAD', { limit: 20 });
console.log(page.oids, page.hasMore);
// After a direct Git import, with its namespace refs already updated:
const signed = await repo.signRefs(signer);
console.log(signed.at, signed.refs);
```

### Policy and routing databases

All operations below return Promises except `.path`. Obtain private-construction
`PolicyStore` handles from `PolicyStore.fromProfile(profile)` (writable),
`PolicyStore.reader(profile)` (read-only), `PolicyStore.open(path)` (writable),
or `profile.policyStore()`. Profile paths use upstream `node/policies.db`.
Factories eagerly run upstream opening/schema checks and close the connection;
queries can still fail later. Writable factories may create their schema/file, but do not create missing parent directories.
Inputs are filesystem paths, not SQLite connection URIs. Empty paths and the
special `:memory:` name reject `ERR_INVALID_ARG_VALUE`: per-operation connections
require a persistent file (Python's held connection can support those SQLite
special names, but they are not part of this Node file-backed contract).
The resulting path is canonicalized once, fixing relative paths and symlink
resolution for later operations. `.path` is that absolute path (unlike Python's
explicit-path spelling); an unrepresentable stored path throws `INVALID_DATA`.

Policy handles retain **only path and access mode**. Each worker call opens,
uses and drops its own connection. Reads always use the upstream read-only
opener, even on writable handles; a missing/incompatible file rejects, with no
writable fallback or schema repair. A permitted write may recreate a deleted
file. Existing handles observe later committed writes/replacement databases,
not a permanently held SQLite connection. A read-only write rejects `READ_ONLY`
after validating its arguments, even if the file has disappeared.

| Policy operation | Result / meaning |
| --- | --- |
| `follow(nodeId, { alias? })`, `unfollow(nodeId)` | upstream changed-state boolean |
| `seed(rid, { scope?: 'all' \| 'followed' })`, `unseed(rid)` | changed-state boolean; default scope `all` |
| `setFollowPolicy(nodeId, 'allow' \| 'block')`, `setSeedPolicy(rid, 'allow' \| 'block')` | changed-state boolean |
| `unblockRid(rid)`, `unblockNid(nodeId)` | remove only block rows |
| `isFollowing(nodeId)`, `isSeeding(rid)`, `isBlocked(nodeId)` | boolean; blocked is not simply unfollowed |
| `followPolicy(nodeId)`, `seedPolicy(rid)` | frozen value or `null` when no explicit row |
| `followPolicies()`, `seedPolicies()` | eager frozen arrays in upstream storage order |
| `nodesByAlias(alias)` | frozen array of readonly `[nodeId, alias]` pairs |

`FollowPolicyInfo` has `nodeId`, optional `alias`, and `policy: 'allow' | 'block'`.
`SeedPolicyInfo` has `rid`, `policy` and optional `scope: 'all' | 'followed'`;
blocked rows omit scope. These are the **raw tables**, not upstream's combined
file-config/effective-policy facade. Fuzzy alias lookup uses upstream SQL
`UPPER`/`LIKE` substring behavior, not general Unicode case folding.
`unfollow`/`unseed` delete block rows too. `seed` changes scope but does not clear
a block policy; the SDK does not invent allow/unblock operations or changed-state
optimizations. Policy store/open/iteration failures reject `POLICY`, not a fake
empty list. Argument domains use the established `ERR_INVALID_ARG_*` codes;
explicit `null` options/alias/scope are not treated as omission.

Policy connections use upstream SQLite busy timeouts (read: 3 seconds; write:
6 seconds), not SDK retries or a JS queue. Concurrent operations/handles/workers
rely on the same addon's SQLite locking and cross-process SQLite locks. They do
not acquire repository/signer mutexes or promise fairness, multi-call atomicity,
read latency, cross-addon coordination or rollback. Do not mix independent SQLite
builds against the same file concurrently within one process (for example
`node:sqlite` and the addon): they have separate in-process VFS lock registries.
External clients should use separate processes and finish writes before expecting
read-your-write results. Connection-open/schema work is part of each operation.

`RoutingStore.fromProfile(profile)` / `profile.routingStore()` eagerly open the
upstream node database read-only. A missing file rejects `DATABASE` and is never
created. Upstream performs connection configuration but runs no migrations;
incompatible databases can fail during opening/configuration or later queries,
without schema repair. A handle retains upstream's
thread-safe shared `Database` connection and clones it for worker jobs. There is
no inventory-write API, daemon access or automatic network discovery.

- `get(rid)` and `getInventory(nodeId)` return sorted frozen ID arrays.
- `entry(rid, nodeId)` returns Date-safe epoch milliseconds or `null` (absent or
  unrepresentable). `entries()` returns frozen `{ rid, nodeId, timestamp }` values,
  sorted by RID then node ID, with required `number | null` timestamps. Oversized
  valid timestamps never hide the entry. Python retains their exact integers.
- `count(rid)`, `total()`, `isEmpty()` return upstream counts/emptiness; unsafe
  integer counts reject `INVALID_DATA`. `entries()` eagerly performs a timestamp
  lookup per pair (N+1 queries), not an atomic snapshot across queries/writers.
- Returned SQLite/decoding errors reject `DATABASE`; malformed argument strings
  reject argument errors. A negative stored timestamp is a decoding failure,
  not an unrepresentable valid timestamp. Some pinned upstream routing queries
  use unchecked row decoding; a corrupt ID can panic and is contained as
  `INTERNAL` by the common worker boundary, not converted into an empty listing.
  Upstream iterator behavior is retained; this is not a general SQL validation API.

### Generic raw COB inspection

`Raw.open(repository)` is a synchronous, lazy, private-construction facade with
`rid` metadata. Its async methods are:

- `typenames()` and `entities(typename)`: sorted, deduplicated frozen strings.
  Enumeration strictly accepts `refs/cobs/<typename>/<id>` after namespace
  stripping. Branch/tag lookalikes and malformed typename/ID components are
  ignored, not allowed to break the whole repository listing. Git backend
  enumeration errors still reject `GIT`; libgit2's handling of malformed loose
  refs is retained. Invalid caller typenames/IDs reject argument errors.
- `refs(typename, entityId)`: name-sorted frozen `CobRef` values with full `name`
  and change-commit `target`. These refs are not independently authenticated;
  authenticity requires signed refs and upstream verification.
- `history(typename, entityId)`: eager frozen `RawOp` values, globally deduplicated
  across all entity ref tips. Traverse **causal parents only**, never related Git
  commits or the identity resource link. Every loaded parent precedes its child;
  the lexicographically-smallest ready ID breaks ties. Missing entities return
  `[]`; load/backend failures reject `COB` (repository-open failures remain
  `REPOSITORY`), never a partial history. A causal cycle rejects `COB`.

A `RawOp` includes `id`, canonical node-ID `actor`, `timestamp`, `actions`,
`parents`, `related`, optional `identity`, `manifest` and `actionsJson()`.
Parents/related are frozen ID arrays; the manifest is `{ typeName, version }`.
`timestamp` preserves upstream's unsigned u64 **bigint epoch seconds**, not milliseconds or a nullable
Date conversion. `manifest.version` is also `bigint`, preserving arbitrary valid
upstream schema versions without a JS-safe-integer restriction. Python exposes
both as integers. Standard `JSON.stringify` needs an explicit BigInt conversion
(e.g. a replacer returning `value.toString()`); no automatic string conversion is
performed by the SDK. RawOp also contains a decoding function: for structured
clone/worker messaging, select its data fields or decoded values rather than
cloning the whole object.

`actions` is a frozen array of independent mutable `Uint8Array` copies, one per
raw action blob. Mutating a view cannot alter storage/another read, but
`actionsJson()` decodes the **current views** of that snapshot. This synchronous
helper returns a frozen array of recursively frozen ordinary JSON values:

- Integer tokens within the safe range are `number`; larger positive/negative
  integer tokens are exact `bigint`, determined from the original token via
  standard JSON reviver source text. Source is supplied for primitives only.
- Decimal/exponent tokens use finite IEEE-754 `number`, including their normal
  rounding/underflow, as Python's float decoder does. For example, `1e30` is a
  number, not a bigint. This is **integer-token fidelity**, not arbitrary-decimal
  losslessness; Python can also round integer tokens beyond its decoder's
  i64/u64 paths, whereas Node preserves those as BigInt. Use original bytes when
  exact lexical spelling/decimal arithmetic matters.
- Invalid JSON/UTF-8, unpaired surrogate escapes, and floating-point overflow
  throw `RadicleError` / `COB`, naming the action index/entry. Raw bytes/history
  remain inspectable even when an action is not valid JSON. The helper does not
  validate action schemas or evaluate typed issue/patch state.

Raw inspection parses change/signature metadata; it is **not a signature or
actor-authentication API** and does not repair corrupt data. It performs no
writes, locking, signing or networking. It reopens the repository per call;
concurrent external ref changes are not an atomic multi-call snapshot. Histories
are eager and not paginated; there is no new resource-isolation guarantee.

### Typed store conveniences

`Issues` and `Patches` expose synchronous `rid` metadata plus async `count()` and
`isEmpty()`. They call the upstream count/emptiness operations, not a binding
`all().length` shortcut, and reopen a read-only store per call. No snapshot is
cached and no operation is recorded. Repository-open failures reject `REPOSITORY`,
store failures reject `COB`; counts outside the JS safe-integer range reject
`INVALID_DATA` rather than round.

Issue snapshots include `commentCount` and frozen `commentIds` in non-redacted
thread order, including the root; these match `comments.length` and the comment
IDs in `comments`. Patch `revisionCount` counts non-redacted revisions by all
authors, matching `revisions.length`, **not** `version + 1` after redactions.

### Issue contract

- `all()` eagerly returns an ID-sorted, frozen array of detached issue snapshots.
  A store/item error rejects the whole call; missing `get(id)` returns `null`.
- Snapshots include `author` (DID), sorted `labels`/`assignees`, and optional
  `closeReason: 'solved' | 'unspecified'` (absent when open), plus discussion.
- `create({ title, description, labels?, assignees? })` returns a handle.
  `label()` and `assign()` **replace** their entire sets; `[]` clears them and
  duplicates are deduplicated upstream. Assignees must be DIDs, not node IDs.
- `editDescription()` edits the root comment, preserving its ID and comment count.
  `comment()` defaults to replying to that root; `replyTo` can target another comment.
- All issue mutations return `{ entryId, snapshot }` under the repository/signing
  locks. These are operation entries, not necessarily comment IDs. `close()` uses
  `unspecified` by default; `close({ reason: 'solved' })` selects the other reason.
  Even repeated lifecycle calls use upstream transactions/entry IDs, not a
  `changed` boolean. Reopening clears the close reason.
- `Issues.open()` is read-only: `create/getMut` reject `READ_ONLY`. Invalid titles,
  labels, DIDs and reasons reject as `TypeError` (`ERR_INVALID_ARG_VALUE`); wrong
  input types use `ERR_INVALID_ARG_TYPE`. Upstream COB failures reject `COB`.
  Issue reads (`all/get/reload/getMut`) do not acquire the write mutex. Upstream
  writes objects before moving refs, so a concurrent read can see an older or
  newer persisted view; listing is not a repository-wide atomic snapshot. Await
  a write before reading its result. No cached snapshot is reused by a mutation.

### Patch contract

- `all()` eagerly returns ID-sorted, frozen detached snapshots. A store/item error
  rejects the whole listing. Reads (`all/get/reload/getMut`) do not take the write
  mutex and are not a repository-wide atomic view, as with issues.
- `create({ title, description, base, head, target?, labels? })` and `draft(...)`
  return handles. Commits must **already exist in storage**; `base`/`head` are
  explicit Git OIDs. Target defaults to `delegates`; `branch:<name>` is also
  supported. Branch input follows upstream ref parsing; snapshot output strips
  one leading `refs/heads/`.
  Empty diffs (`base === head`) are accepted, matching upstream.
- Snapshots contain author DID, state (`draft | open | archived | merged`),
  target, sorted labels/assignees, revisions and their comments/reviews. Top-level
  `description/base/head/latestRevision` select the **patch author's** latest
  revision, not the last revision by any author. `version` is upstream's slot
  count minus one (including redacted slots), **not an array index**; find a
  revision by ID. Redacted revisions are absent from `revisions`.
- `edit({ title, target })`, replacement `label/assign` (`[]` clears), revision
  `comment(revision, body, { replyTo? })`, `review(revision, { verdict?, summary? })`
  and `merge(revision, commit)` return `{ entryId, snapshot }`. Verdicts are
  `accept | reject`; omit verdict for a summary-only review. An empty review is
  rejected upstream. Upstream keeps the first review per actor/revision; another
  `review()` records an operation but does not replace it. Entry IDs identify
  operations, not necessarily an added/changed object. Review editing is deferred.
- `update({ description, base, head })` returns `{ revisionId, snapshot }`, adds
  an upstream revision, and signs refs. It does not import commits, move a patch
  branch, deduplicate base/head pairs, automatically merge, or sync.
- `ready/unready/archive/unarchive` return `{ changed, snapshot }`. The boolean
  is the **upstream transaction decision**, not a before/after state comparison.
  Upstream no-ops return false without a transaction; repeated `archive()` calls
  return true and record transactions even when already archived.
- Every result is materialized under the repository/signing locks after a fresh
  load. Read-only `Patches.open()` rejects `create/draft/getMut` as `READ_ONLY`.
  Invalid argument values/types and COB failures follow the issue error contract.
  No added rollback, retry, snapshot cache or patch-specific scheduler.
- `merge()` records a COB merge decision, **never a Git merge or branch move**.
  Upstream evaluates the supplied revision/commit against the delegate target
  branch. A recorded operation can be ignored; inspect `snapshot.state` and the
  optional `mergedRevision`/`mergedCommit` rather than assuming success means merged.

Programmatic example, with `base`, `head` and `nextHead` already in storage:

```ts
import { Patches } from 'radicle-sdk';
const patches = Patches.openAs(repo, signer);
const patch = await patches.draft({ title: 'Change', description: 'First', base, head });
const initial = await patch.reload();
await patch.ready();
await patch.comment(initial.latestRevision, 'Discussion');
const revision = await patch.update({ description: 'Second', base, head: nextHead });
const reviewed = await patch.review(revision.revisionId, { verdict: 'accept', summary: 'Approved' });
console.log(reviewed.entryId, reviewed.snapshot);
// After Git has put the supplied commit on the delegate target branch:
const merged = await patch.merge(revision.revisionId, nextHead);
console.log(merged.snapshot.state); // inspect, do not assume 'merged'
```

For **working-copy patch creation/update**, prefer the Git remote helper:

```sh
# Open a new patch (ASCII, single-line push-option messages):
git push rad HEAD:refs/patches -o no-sync -o 'patch.message=Change' -o 'patch.message=Description'
# Update an existing patch, with PATCH_ID set to its ID:
git push rad "HEAD:refs/heads/patches/$PATCH_ID" -o no-sync
```

Unlike SDK `create/draft/update`, these pushes import commits and maintain the
patch branch. Both paths default the base to the merge-base with canonical head
(`-o patch.base=<oid>` overrides it). Creation rejects empty patches; updating
skips an existing base/head pair and can record an open patch as merged when
base equals head. Without `no-sync`, the helper attempts sync only when seeding
and the node is running; sync failure does not fail the local push. SDK calls
never sync or create/update `refs/heads/patches/<id>`. They **do sign refs**, and
revision base/head remain reachable as COB Git parents without a patch branch.
There is no SDK commit-import API or merge-base default.

## Concurrency and environment boundaries

- Repository/COB writes: a Rust registry shares locks by canonical storage path + RID across handles
  and `worker_threads` loading the same addon. Write-side reload, transaction,
  signing and result snapshot run under that lock. Reads remain parallel and do
  not wait on the write mutex; repository handles are reopened inside each task.
- Signer guard follows the repository lock and is retained for the upstream
  transaction. Signer getters use cached public metadata and never wait on it.
- Policy/routing use their database contracts above, not the repository mutex.
  SQLite busy waits can also occupy worker threads. No cross-database or
  repository-plus-policy transaction is provided.
- No JS write queue. Waiting writers occupy threads in Node's worker pool and can
  delay reads;
  no ordering/fairness or read-latency guarantee is made.
- External processes, CLI/Git writes, and separate loaded copies/versions of the
  addon are not coordinated. Init has no RID yet; concurrent init of one working
  directory, and concurrent checkouts to one destination, are not supported.
- `Profile.load()` and `profile.signer()` call upstream synchronously on the
  calling JS thread, like Python. Filesystem reads, key decryption and agent
  communication can block that thread; these methods return handles, not Promises,
  and throw typed errors synchronously. Repository workflows remain asynchronous.
- Upstream resolves `RAD_HOME`/`HOME`, `RAD_PASSPHRASE` and `SSH_AUTH_SOCK` from the
  native OS process environment. Node per-worker `env` copies do **not** select
  different profiles or signers. Set environment before starting the process;
  do not mutate it concurrently or use `SHARE_ENV` to multiplex profiles.
  No custom loader, signer adapter or temporary native environment mutation.
- Tests cover encrypted/plaintext keys and a throwaway Unix SSH agent with local
  project/issue writes. Other agent implementations/hardware are unverified.
  There is no node networking or remote fetch/sync API. Errors do not imply rollback.

## Panic containment

Worker jobs catch Rust unwinding panics and reject with `RadicleError` code
`INTERNAL`, without returning the panic payload. Synchronous native entry points
also use native panic guards. Panic hooks may still write diagnostics to stderr.
A poisoned mutex rejects while it remains alive, but this is **not a permanent
repository quarantine**: the registry holds weak references, so once the last
owner drops, another operation can acquire a fresh mutex and write again. A
poisoned signer mutex remains poisoned for handles sharing that signer; loading
a new signer creates a new mutex. Operations are not automatically retried or
rolled back. Inspect persisted state before deliberately continuing after an
internal failure. `panic=abort` builds
are rejected; aborts, double panics during unwinding, OOM and native faults cannot
be contained this way. This is not process isolation.

## Timestamp conversion

Stored timestamps (`Comment.timestamp`, `Revision.timestamp`, `Review.timestamp`,
`Commit.authorTime`/`committerTime`) are
required `number | null` fields. Checked conversion returns exact epoch milliseconds
inside the ECMAScript Date range, or `null` outside it (including overflow), never
a rounded number. Unrepresentable peer metadata does not hide an issue/patch, reject a
listing or prevent a mutation; genuine store/decoding errors still reject.

The public SDK has no timestamp-input API. Raw COB timestamps are the explicit
exception to Date conversion: exact bigint seconds, with bigint manifest versions;
see generic raw inspection above.

## Maintainer notes

Consumers do not need this section.

- The timestamp and panic probes (`node/src/probes.ts` and the native `probe*`
  functions) are test-only leftovers from the initial native baseline, not
  package exports. Their explicit inputs keep range and validation rejection
  so the error-translation layer stays covered by `node/test/timestamps.test.mjs`
  and `node/test/panics.test.mjs`.
- Worker tasks run on Node's libuv thread pool; the lock registry lives in
  `node/native/src/locks.rs` and the panic containment in `tasks.rs`.
