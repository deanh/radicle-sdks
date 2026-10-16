# radicle-sdk: TypeScript API contract

The current `0.1.0` alpha contract for Node.js/TypeScript, pinned to upstream
Radicle `=0.25.1`. The full reviewed Git SHA identifies a candidate. See the
[guide](README.md) for installation and an executable quick start, and the
[changelog](CHANGELOG.md) for package history. Only `radicle-sdk` is exported;
native bridge and probe files are internal, not supported import paths.

## Implemented surface

- Async `Profile.init`; synchronous `Profile.load()`/`signer()` setup and profile
  metadata/storage handles.
- Lazy `storage.repository(rid)`; async storage `contains/repositories`.
- Async repository `head/identity/branches/branchHead/commit/tree/blob/history`,
  plus explicit `signRefs(signer)`.
- Async `initProject` (public by default, or `visibility: 'private'`),
  `forkProject`, `checkoutProject` (non-bare only; bare checkout remains deferred).
- Lazy `Issues.open/openAs`; async `all/get/create/getMut`.
- Mutable issue handle with `id`, async `reload/comment/edit/editDescription`,
  `label/assign/close/reopen`.
- Lazy `Patches.open/openAs`; async `all/get/create/draft/getMut`.
- Mutable patch handle with `id`, async `reload/edit/label/assign/update`,
  `ready/unready/archive/unarchive`, revision `comment/review`, and merge records.
- Generated readonly DTOs, literal state/visibility domains and copied byte fields.
- Real `RadicleError` instances/codes and argument-error translation.

Generated declarations describe exact consumer types; the behavioral guarantees
and limits below supplement those types rather than promising upstream parity.

## Handle and snapshot contract

Handles have private constructors in the emitted consumer declarations and reject
direct JavaScript construction. Use `Profile.init/load`, `profile.signer()`,
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
has no general rollback guarantee. Checkout remains non-bare.

Snapshots/result envelopes recursively freeze ordinary objects/arrays. Byte
fields are independent, mutable `Uint8Array` copies; changing one cannot alter
storage or another snapshot. Handles cache no snapshots. A comment result pairs
its entry ID with the post-operation snapshot built under the write lock.
Missing `Issues.get`/`Patches.get` returns `null`; missing repositories fail at first I/O.
This alpha exposes the bounded surface above, not the full upstream crate.

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
- Reads remain parallel; signing shares the write lock with issue/patch writes
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

- A Rust registry shares locks by canonical storage path + RID across handles
  and `worker_threads` loading the same addon. Write-side reload, transaction,
  signing and result snapshot run under that lock. Reads remain parallel and do
  not wait on the write mutex; repository handles are reopened inside each task.
- Signer guard follows the repository lock and is retained for the upstream
  transaction. Signer getters use cached public metadata and never wait on it.
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

The public SDK has no timestamp-input API and no raw/BigInt representation.

## Maintainer notes

Consumers do not need this section.

- The timestamp and panic probes (`node/src/probes.ts` and the native `probe*`
  functions) are test-only leftovers from the initial native baseline, not
  package exports. Their explicit inputs keep range and validation rejection
  so the error-translation layer stays covered by `node/test/timestamps.test.mjs`
  and `node/test/panics.test.mjs`.
- Worker tasks run on Node's libuv thread pool; the lock registry lives in
  `node/native/src/locks.rs` and the panic containment in `tasks.rs`.
