# radicle-sdk: TypeScript alpha (in progress)

**Experimental, incomplete TypeScript v0.** Builds on the isolated T0 spike;
local issue and patch COB workflows are implemented; supporting repository
expansion remains separate.
The Python package and its root Git-install path are unchanged. No shared core
has been extracted.
The native crate lives in `node/native`; the npm manifest is at repository root
so Git dependencies can build it without unsupported subdirectory fragments.

## Build and test

Requires Node **26.12.0+** (tested baseline: 26.12.0), **Rust ≥ 1.91**, a C
compiler, and Git. The pinned radicle 0.25.1 declares Rust 1.88.0 but doesn't
compile on 1.88–1.90; see `FINDINGS-TS-T0.md`. Node 26 is Current on 2026-10-15 and is scheduled for Active
LTS on 2026-10-28. npm publication and prebuilt platform packages are deferred.
The working package name is `radicle-sdk`; registry availability isn't assumed.

```sh
nvm use
npm ci --ignore-scripts
npm run build
npm run check
npm test
cargo test --manifest-path node/native/Cargo.toml --locked
cargo clippy --manifest-path node/native/Cargo.toml --all-targets --locked -- -D warnings
```

`prepare` builds the Rust addon with the committed Cargo lockfile, generates
native TypeScript declarations, then compiles the thin TS wrapper. This alpha uses a
debug native build, not a release/performance or portable-binary claim.
Do not regenerate the lockfiles casually: the standalone feature graph needs
explicit `indexmap/std` for pinned radicle 0.25.1.

## Alpha source installation

Use a Git URL supported by your Git client; the package is at repository root:

```sh
# npm 12+ requires explicit approval for root Git dependencies:
npm install --allow-git=root "git+${SDK_GIT_URL}#<full-commit>"
# npm 11 (does not have the Git opt-in policy):
npm install "git+${SDK_GIT_URL}#<full-commit>"
# pnpm 12: first approve the exact Git source in pnpm-workspace.yaml (below).
pnpm add "git+${SDK_GIT_URL}#<full-commit>"
# Bun Git installs are best effort; see the observed limitation below.
bun add --trust "git+${SDK_GIT_URL}#<full-commit>"
```

For pnpm 12, add an exact-source entry to the consuming project's
`pnpm-workspace.yaml`, replacing the URL and commit with literal values:

```yaml
allowBuilds:
  "radicle-sdk@git+https://YOUR-HOST/YOUR-REPOSITORY.git#FULL_COMMIT_SHA": true
```

`--allow-build radicle-sdk` alone did not approve Git `prepare` in the tested
pnpm release. The smoke script writes the exact-source approval automatically.
Only trust build scripts from reviewed commits. Rust/C/Git must be available to
the installing process; installing a JS dependency compiles native code.
The package has `private: true` to prevent accidental registry publication;
this does not prevent Git installs.

Observed Bun 1.4.3 limitation: Git `prepare` runs without installing this
package's development build tools, so the command above fails with `napi: command
not found`. npm/pnpm are the validated source-install paths. Bun can load and run
the Node-built addon; that is not a claim that Bun's Git build path works.

After committing, validate the actual root Git dependency in a clean project:

```sh
bash node/scripts/git-install-smoke.sh npm    # adds Git approval for npm >=12
bash node/scripts/git-install-smoke.sh npm12  # pinned npm 12.2.0 gate
bash node/scripts/git-install-smoke.sh pnpm
bash node/scripts/git-install-smoke.sh bun  # best effort
```

These checks default to `git+file://...#HEAD`, isolate package-manager caches,
and reuse compiled Cargo dependencies only (the repository contains no built
addon). Set `SDK_GIT_URL` to test a different clone transport. The smoke runs
init/inspect/fork/checkout and the issue/patch workflows using installed exports.
No publication is involved. `npm install /path/to/checkout` is a manual fallback,
not a replacement for the Git-install gate.

## Implemented surface

- Async `Profile.init`; synchronous `Profile.load()`/`signer()` setup and profile
  metadata/storage handles.
- Lazy `storage.repository(rid)`; async repository `head`, `identity`, `commit`.
- Async `initProject` (public project), `forkProject`, `checkoutProject`.
- Lazy `Issues.open/openAs`; async `all/get/create/getMut`.
- Mutable issue handle with `id`, async `reload/comment/edit/editDescription`,
  `label/assign/close/reopen`.
- Lazy `Patches.open/openAs`; async `all/get/create/draft/getMut`.
- Mutable patch handle with `id`, async `reload/edit/label/assign/update`,
  `ready/unready/archive/unarchive`, revision `comment/review`, and merge records.
- Generated readonly DTOs, literal state/visibility domains and copied byte fields.
- Real `RadicleError` instances/codes and argument-error translation.

For an existing Git working copy with a committed `main` branch:

```ts
import { Profile, initProject, Issues } from 'radicle-sdk';

const profile = await Profile.init('/new/rad-home', 'alice');
const signer = profile.signer();
const rid = await initProject(profile.storage, signer, '/existing/work', {
  name: 'hello', description: 'Demo', defaultBranch: 'main',
});
const repo = profile.storage.repository(rid);
const issue = await Issues.openAs(repo, signer).create({
  title: 'First', description: 'Root', labels: ['bug'], assignees: [profile.did],
});
const { entryId, snapshot } = await issue.comment('A reply');
await issue.edit('Clarified title');
await issue.editDescription('Clarified description');
await issue.label(['triaged']);
await issue.assign([]);
const closed = await issue.close({ reason: 'solved' });
await issue.reopen();
const all = await Issues.open(repo).all();
console.log(entryId, snapshot.comments, closed.snapshot.closeReason, all);
```

Snapshots/result envelopes recursively freeze ordinary objects/arrays. Byte
fields are independent, mutable `Uint8Array` copies; changing one cannot alter
storage or another snapshot. Handles cache no snapshots. A comment result pairs
its entry ID with the post-operation snapshot built under the write lock.
Missing `Issues.get`/`Patches.get` returns `null`; missing repositories fail at first I/O.
This alpha intentionally exposes only a subset of the planned v0 fields/methods.

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
- No JS write queue. Mutex waiters can occupy libuv pool threads and delay reads;
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
also use napi-rs panic guards. Panic hooks may still write diagnostics to stderr.
A poisoned mutex rejects while it remains alive, but this is **not a permanent
repository quarantine**: the registry holds weak references, so once the last
owner drops, another operation can acquire a fresh mutex and write again. A
poisoned signer mutex remains poisoned for handles sharing that signer; loading
a new signer creates a new mutex. Operations are not automatically retried or
rolled back. Inspect persisted state before deliberately continuing after an
internal failure. `panic=abort` builds
are rejected; aborts, double panics during unwinding, OOM and native faults cannot
be contained this way. This is not process isolation.

Stored timestamps (`Comment.timestamp`, `Revision.timestamp`, `Review.timestamp`,
`Commit.authorTime`/`committerTime`) are
required `number | null` fields. Checked conversion returns exact epoch milliseconds
inside the ECMAScript Date range, or `null` outside it (including overflow), never
a rounded number. Unrepresentable peer metadata does not hide an issue/patch, reject a
listing or prevent a mutation; genuine store/decoding errors still reject.

T0-only timestamp/panic probes are internal files, not package exports. Explicit
timestamp probe inputs retain range/validation rejection. The public SDK does
not gain a timestamp-input API or a raw/BigInt representation.
