# TypeScript SDK proposal

Status: **T0 locally demonstrated on `ts-t0`; findings await an architecture decision**. Goal: a
programmatic TypeScript interface to the same core local Radicle workflows
radicle-py covers, wrapping the `radicle` crate. The default answer to any extra
feature is "not until a workflow needs it". rad-pi (the pi extension for
Radicle) is the **reference consumer for scope decisions**. It is not a gate and
not a reason to add features.

Next step: **T0 spike → findings → explicit architecture decision** (§7). Core
extraction and a conformance framework are *candidates* for that decision, not
foregone conclusions. T0 results are in [`FINDINGS-TS-T0.md`](./FINDINGS-TS-T0.md):
Linux workflows, concurrency, types and npm/pnpm Git installs pass. The pinned
upstream's declared Rust 1.88 fails; the measured minimum is 1.91. Bun's own Git prepare fails while its
runtime can use the Node-built addon. These limitations must inform the next
decision. No core extraction or full v0 implementation has been authorized.

### Selected package and runtime

- Working package name and import specifier: **`radicle-sdk`**. Registry
  availability has not been checked; reservation and npm publication are deferred.
- Supported runtime for T0/v0: **Node.js 26**, chosen ahead of its scheduled
  transition to **Active LTS on 2026-10-28**. It is still Current as of
  **2026-10-15**; do not describe it as LTS before that transition. The latest
  Node 26 release in the official index is **v26.12.0**; use that exact version
  for the initial spike and record the tested version in findings/CI.
  Initial package engines: `>=26.12.0`, with **no upper bound**. Node-API is
  ABI-stable across Node majors, so a cap would only cause install warnings
  (or failures under `engine-strict`) for newer Node versions without any
  technical reason. *Support* is still stated as "tested on Node 26". Follow
  supported patch/minor updates within Node 26, including validating an LTS
  release once available. Moving the tested baseline to another major is an
  explicit decision, not a floating `lts/*` CI target.
- Bun remains best effort; older Node LTS majors, Deno and browsers are not
  supported targets. Rust stays pinned to `radicle = "=0.25.1"`. **Minimum Rust
  is 1.91**: upstream declares 1.88.0 but doesn't compile on 1.88–1.90
  (`cob/identity.rs`, see `FINDINGS-TS-T0.md`). Both SDKs declare 1.91;
  re-measure on every radicle pin bump.

## 1. Is a new SDK justified?

| Option | Covers | Missing |
|---|---|---|
| `radicle-httpd` JSON API (used by radicle-explorer) | Remote/local **reads** | Read-only by design and needs a running daemon. No local signing, init, issues or patch writes |
| Shelling out to `rad` | Most workflows | Human-oriented CLI: output isn't a stable API, has interactive prompts, needs `rad` installed |
| npm packages (`jsradicle` etc.) | — | Experimental or unrelated; none wrap the crate |

**Conclusion:** a TypeScript SDK is justified for **typed local reads and signed
writes**. Read-only, browser or remote consumers should be pointed at
`radicle-httpd`.

### What rad-pi shows

`@rad-pi/core@1.6.1` (`extensions/radicle-tools.ts`, 21 tools) shells out to
`rad` and parses **plain-text** output (no `--json`; patch IDs are pulled out
with a regex). Its tools fall into three groups:

| rad-pi need | v0 status |
|---|---|
| Issue list/show/comment/state/labels; patch list/show/review/assign; `self`; `inspect --delegates` | **Covered.** Typed snapshots would replace the text parsing, which is the clearest value |
| `sync`, `clone`, `node status`, remote management | **Out of scope by design** (network/node). rad-pi keeps the CLI for these |
| Patch creation/update from a working branch (distinct Git refspecs below) | **Stays with `git push`.** This is the documented, recommended path; the SDK doesn't replace it |
| Patch diff, patch checkout | Deferred |
| `rad init` | Not used by rad-pi; kept for parity with the alpha workflows |

### Patch creation: `git push rad` vs. SDK `create()`

For working-copy flows, use **`git push rad HEAD:refs/patches` to open** a
patch and **`git push rad HEAD:refs/heads/patches/<id>` to update** it. The SDK
docs will distinguish them. For creation, the remote helper
(`radicle-remote-helper` 0.18.1, `push.rs`) does the following in order:

1. copies the commits into storage;
2. defaults `base` to the merge-base with the canonical head (`patch.base` can override it);
3. rejects empty patches;
4. calls the same `Patches::create`/`draft` the SDK wraps;
5. writes `refs/namespaces/<nid>/refs/heads/patches/<id>`;
6. signs refs;
7. attempts sync when seeding and the node is running, unless `no-sync`;
   sync failure does not fail the local push.

SDK `create()`/`draft()` remain for **programmatic flows where the commits are
already in storage**. They differ from the push path in these ways, all of which
must be documented:

| Step | `git push rad` | SDK `create()` |
|---|---|---|
| Get commits into storage | yes | caller's job |
| Default `base` | merge-base with canonical head | must be passed explicitly |
| Empty patch (`base == head`) | rejected | accepted |
| `refs/heads/patches/<id>` | created | not created |
| Sign refs | yes | yes (COB writes sign) |
| Node sync | conditional, best effort (as above) | never |

The push update path uses the same default-base calculation, skips an existing
base/head pair, and can record an open patch as merged when `base == head`.
SDK `update()` adds a revision with an explicit base; it does not move the patch
branch, automatically record a merge, or sync. Its commits must already be in
storage, and its COB transaction signs refs.

The missing patch branch is **not** a correctness problem. Revision actions record
`base`/`head` as `related` (`radicle` 0.25.1 `cob/patch.rs:383`,
`cob/store.rs:277`), and those become Git parents of the COB change commit
(FINDINGS-0A). So the patch's commits replicate with the COB and are safe from
garbage collection. The branch is a checkout and tracking convenience.

Not added in v0: a commit-import API, a merge-base default, or an empty-patch
check. Each is a candidate if programmatic users hit the difference; the
documentation comes first.

Caveat: this is based on one file of rad-pi; helpers in `lib/rad-shared.ts`
weren't inspected.

## 2. Scope (v0): the three alpha stop-rule workflows

1. Git repository → init → inspect/fork → local checkout.
2. Issue create → discussion → assign/label → close/reopen.
3. Patch draft → ready → new revision → comment/review → merge record.

| Area | Surface |
|---|---|
| Profile/signer | `Profile.load()`, `Profile.init(home, alias, {passphrase})`, `profile.signer()`, `did`, `nodeId` |
| Storage/repo reads | `repositories()`, `contains()`, `repository(rid)`, `head()`, `identity()`, `branches()`, `branchHead()`, `commit()`, `tree()`, `blob()`, `history()` |
| Project | `initProject`, `forkProject`, `checkoutProject` (with the init `rad` remote preflight) |
| Issues | open/openAs, `all/get`, `create`, `getMut`; `comment`, `edit`, `editDescription`, `label`, `assign`, `close`, `reopen` |
| Patches | open/openAs, `all/get`, `create/draft`, `getMut`; `edit`, `label`, `assign`, `update`, `ready/unready`, `archive/unarchive`, `comment`, `review`, `merge`; `latestRevision` |
| Repo writes | `signRefs(signer)` only (needed after a direct commit import) |

**Deferred:** `PolicyStore`, `RoutingStore`, `Aliases`, `Home`, `Raw` COB
inspection, `tags()`/`remotes()`/`revParse()`, the other three repository
writes, diffs, patch checkout, commit import API, sync API variants, browser
support, network/node control, npm publication.

## 3. Integration approaches

The crate links **C libraries** (`git2`/libgit2, bundled `sqlite`), shells out
to the **`git` executable** (init, checkout), and uses the **filesystem** and
**Unix sockets** (ssh-agent).

### A. Native Node-API addon via napi-rs (the direct analogue of PyO3/maturin)

An in-process `.node` library. `#[napi]` classes for handles, `#[napi(object)]`
for snapshots, and TypeScript declarations generated from the Rust source.

- **+** Same architecture as radicle-py, so semantics port one-to-one.
- **+** Generated `.d.ts` reduces stub drift. It does not prove runtime
  behaviour (§4 lists what T0 must still check).
- **+** I/O runs off the JS thread (`AsyncTask`, libuv pool) and resolves to Promises.
- **+** Bun implements Node-API natively (napi-rs: best effort).
- **−** One binary per platform (the same issue as PyPI, also deferred).
- **−** No Deno support target and no browser support.
- **−** Real concurrency (§5).

### B. C ABI + JSON via each runtime's FFI (koffi / `bun:ffi` / `Deno.dlopen`)

- **+** One library for Node, Bun and Deno.
- **−** Memory ownership and error/string protocol written by hand across the
  boundary; no natural async; types maintained twice; three FFI test matrices.

### C. Sidecar process: a Rust binary speaking JSON-RPC over stdio

- **+** Crash isolation; runs on any JS runtime; async for free.
- **−** Still a per-platform binary. Adds process lifecycle, a versioned protocol,
  signer handoff, and server-side sessions for `openAs`/`*Mut` handles.
  It's effectively another daemon.

### D. Rejected without a spike

WebAssembly (C libraries, subprocesses, agent socket), and the `rad` CLI or
httpd wrappers covered in §1.

### Recommendation: A (napi-rs)

It's the only option that keeps radicle-py's property of a **thin, in-process
wrapper with one-to-one upstream semantics**. Revisit C only for a non-Node
runtime need or demonstrated crash-isolation needs.

## 4. TypeScript API shape

**Semantic parity, idiomatic surface.** Operations, validation, error categories
and upstream behaviour match radicle-py; naming and async follow TypeScript.

```ts
import { Profile, initProject, Issues } from "radicle-sdk";

const profile = Profile.load();
const signer = profile.signer();
const rid = await initProject(profile.storage, signer, "/path/to/work", {
  name: "hello", description: "Demo", defaultBranch: "main",
});
const repo = profile.storage.repository(rid);         // handle: sync, no I/O
const head = await repo.head();                        // I/O: always async method
const issue = await Issues.openAs(repo, signer).create({ title: "First", description: "…" });
const reply = await issue.comment("A reply");
console.log(reply.entryId, reply.snapshot.comments);
const closed = await issue.close({ reason: "solved" });
console.log(closed.snapshot.state);
const current = await issue.reload();                  // explicit fresh read
```

- **Handles vs. I/O.** Constructing handles (`storage`, `repository(rid)`,
  `Issues.open/openAs`) is **synchronous and does no I/O**, matching radicle-py's
  lazy stores. Repository reads/writes are **async methods**, never I/O-bearing
  getters (`repo.head()`, not `repo.head`). Getters are only for data already
  held in memory (`did`, `nodeId`, snapshot fields). Unlike Python,
  `repository(rid)` doesn't check existence; the first I/O call fails instead.
  **Setup exception:** `Profile.load()` and `profile.signer()` synchronously call
  upstream on the calling JS thread, matching Python without duplicating profile
  or signer logic. Filesystem access, decryption and agent communication can
  block; errors throw synchronously. `Profile.init()` remains async. Environment
  resolution uses native OS process values, not Node per-worker `env` copies;
  configure them before process startup and do not mutate them concurrently.
- **Snapshots** are detached plain objects. Recursively freeze ordinary object
  and array containers with `Object.freeze`, and generate readonly properties
  and readonly collection types. **Byte arrays are the explicit exception:**
  nonempty `Uint8Array` instances cannot be frozen in Node. Their contents stay
  mutable, while the containing property is readonly/non-reassignable. Every
  returned byte field owns an independent copy: modifying it cannot affect
  Rust/storage, another field, or a later snapshot. This is an intentional
  difference from Python's immutable `bytes`; no proxies or new byte classes.
  No live views or mutable handle caches are exposed.
- **Options objects** replace long positional string lists:
  `create({ title, description, base, head, target = "delegates" })`.
- **Literal unions** for states, verdicts and visibility, generated as finite
  unions and checked in T0 so they don't widen to `string`.
- **Bytes:** Git names/messages as copied `Uint8Array`, with lossy `…Str`
  conveniences where Python has them. The recursive freezer skips typed-array
  views; it must not call `Object.freeze` on their nonempty contents. TypeScript
  readonly properties do not claim that the byte elements are immutable.
- **Timestamps:** epoch **milliseconds** as `number`, converted in Rust with
  **checked arithmetic**. Upstream seconds use `checked_mul(1000)`. A
  value is exposed only if it lies within the **ECMAScript `Date` range**
  (±8.64 × 10¹⁵ ms, which is narrower than `Number.MAX_SAFE_INTEGER`), so every
  exposed timestamp is exact and works with `new Date(ms)`. Out-of-range
  peer-controlled metadata makes the operation reject with
  `RadicleError{code:"INVALID_DATA"}`. It never wraps, rounds or becomes BigInt.
  Lossless raw values are deferred until something needs them.
- **Errors:** `RadicleError extends Error` with a typed `code` (`"STORAGE" |
  "REPOSITORY" | "GIT" | "IDENTITY" | "SIGNER" | "COB" | "READ_ONLY" |
  "INVALID_DATA"`), also for rejections from worker tasks. Invalid arguments use
  `TypeError`/`RangeError` with Node-style `code`s. T0 verifies these are real
  instances with stable codes, not just declared types.
- **Paths:** `string` in v0.

### Mutation results (selected contract)

`IssueMut`/`PatchMut` are ID + repository + signer handles, not mutable snapshot
containers. They have no cached `.issue`/`.patch` properties. Their `id` getter
is synchronous; `reload()` returns `Promise<Issue>`/`Promise<Patch>` from storage.
Store `get()` returns a snapshot; async `getMut()` validates/loads the entity and
returns a handle. `create()`/`draft()` remain factories returning handles, not
mutation-result envelopes; read their snapshots explicitly with `reload()`.

Entity mutations return the operation result **and the post-operation snapshot
from the same locked operation**, rather than requiring an extra racing read:

| Methods | Promise result |
|---|---|
| Issue `comment`, `edit`, `editDescription`, `label`, `assign`, `close`, `reopen` | `IssueChange`: `{ readonly entryId: string; readonly snapshot: Issue }` |
| Patch `edit`, `label`, `assign`, `comment`, `review`, `merge` | `PatchChange`: `{ readonly entryId: string; readonly snapshot: Patch }` |
| Patch `update` | `PatchRevisionChange`: `{ readonly revisionId: string; readonly snapshot: Patch }` |
| Patch `ready`, `unready`, `archive`, `unarchive` | `PatchStateChange`: `{ readonly changed: boolean; readonly snapshot: Patch }` |

These are small, concrete generated object types, not a generalized result
framework. Freeze result envelopes with the same container/byte policy. On a
no-op state transition, `changed` is false and the snapshot is the state read
under the lock; do not invent an operation ID or add a transaction. Otherwise
preserve the upstream commit-per-operation behavior. Errors reject the Promise
rather than returning a success envelope; upstream partial failures do not gain
rollback guarantees.

This intentionally differs from Python's scalar mutation results and cached
handle snapshots while preserving upstream operation semantics. A result is a
snapshot at that operation, not a promise that no later write has occurred.
T0 proves issue `comment`'s envelope and concurrent snapshot isolation; T2 tests
the remaining result shapes as their methods are implemented.

## 5. Concurrency contract

Send/Sync facts for `radicle =0.25.1` (to be recorded reproducibly in T0's
FINDINGS file, not only cited from an earlier compile check):

| Type | Send | Sync |
|---|---|---|
| `profile::Signer` | yes | **no** (`RefCell<ssh::agent::Agent>`) |
| `Profile`, `storage::git::Storage` | yes | yes |
| `storage::git::Repository` | yes | **no** (raw libgit2 pointer) |

v0 contract:

1. **Write lock key = canonical storage path + RID**, in a process-wide registry.
   All handles to the same stored repository share it, however they were opened.
2. **Lock scope = the whole operation:** reload the entry, apply the transaction,
   sign, and build the snapshot that is returned. Nothing mutable is read before
   acquiring the lock. Each call returns its *own* snapshot and handles cache
   nothing, so out-of-order Promise callbacks can't publish stale state.
   Reload-per-transaction (radicle-py `issue_tx!`/`patch_tx!`) and
   commit-per-operation are preserved.
3. **Lock order:** repository lock, then signer mutex
   (`Arc<Mutex<profile::Signer>>`). For T0, the signer guard may be held for the
   full upstream transaction if `WriteAs` requires that borrow. Do not build a
   signer adapter merely to shorten this scope; record the actual scope and
   contention in findings. No operation holding a signer guard may then acquire
   a repository lock.
4. **Coverage boundary:** binding locks don't coordinate other processes, the
   `rad` CLI, external `git`, or other SDKs. A consumer mixing SDK writes with
   `rad` CLI writes (as rad-pi would during migration) gets no ordering guarantee.
5. **The Rust lock is mandatory.** Acquire the shared per-repository mutex
   inside `compute()` and retain it across the complete operation. It must
   coordinate independently opened handles, including handles in different
   Node `worker_threads`/JS realms within the process. Each worker opens its own
   handle; handles themselves need not be transferable between JS realms.
6. **Read parallelism isn't guaranteed.** Waiting writers can occupy libuv pool
   threads (default 4) and delay reads. T0 measures contention with the Rust-lock
   baseline. A realm-local async JS queue before task submission is a possible
   later optimization **in addition to**, never instead of, that Rust lock.
   It cannot provide cross-realm correctness. No custom scheduler or JS queue
   in the initial implementation without measured need.

T0 tests concurrent writes through the **same** mutable handle and through
**separate** handles, including a failing operation in the middle (the lock is
released, later writes succeed), persisted results after a reload, and that
each returned snapshot is fresh. Also open independent handles in multiple
`worker_threads` and verify that they share repository serialization. Assert
that a returned comment's `entryId` is present in its own snapshot, that earlier
snapshots remain unchanged, and that final persisted state includes every
successful operation; do not assume `Promise.all` submission order determines
lock acquisition order.

## 6. Mono-repo or separate repositories?

Not decided until after T0 (§7). The analysis to decide it with:

**For co-development.** Upstream pin bumps are the main thing that keeps the
SDKs in sync, and they're frequent: Radicle 1.7.0 (March 2026) shipped crate
0.21.0, and radicle-py now pins 0.25.1. Semantic fixes (the init `rad`
preflight, `latest_revision`, strict COB ref parsing, tag peeling) would
otherwise be made twice.

**Against.** Two toolchains in one repo. Moving the Rust package risks
radicle-py Git and sdist installs. Release cadence becomes coupled. Mixed
audiences share one Radicle tracker. And which abstractions are worth sharing
isn't known until a second binding exists.

**If T0 supports extraction, T1 must preserve and validate:**
- install from an exact Git commit at the repository root (`pyproject.toml` stays at root);
- a self-contained sdist (including the shared core's sources) that installs;
- abi3 and CPython 3.9;
- installed exports, stubs, package/runtime version checks and release-helper exclusion;
- every existing Python API and behavioural contract on **installed artifacts**,
  not just source-tree tests.

Other conditions:
- **One upstream version:** a single exact `[workspace.dependencies] radicle =
  "=0.25.1"` inherited by members, or the core as sole owner of `radicle`. A
  shared lockfile alone isn't enough: Cargo can lock semver-incompatible duplicates
  (here partly limited by libgit2-sys's and sqlite3-src's `links` keys).
- **Tags and CI:** CI triggers on `v*` and `tests/wheel_smoke.py` asserts
  `tag == v{package_version}`. Per-package prefixes (`py-v*`, `ts-v*`) need both
  changed. Path-filtered CI must run **both** bindings when the core, workspace
  or packaging inputs change.

## 7. Plan

**Post-T0 decision:** proceed with bounded TypeScript v0 increments in the
isolated native crate, **without T1/shared-core extraction**. Patch 1 completes
issues (listing, creation metadata, edits, labels/assignment, close/reopen), using
upstream transactions and locked `{ entryId, snapshot }` results. Patch workflows
and supporting repository reads remain separate follow-ups, not part of patch 1.
No Python restructuring, scheduler, networking or publication work is authorized
by this increment. Current implemented surface and contracts: `node/README.md`.

| Phase | Work | Exit criteria |
|---|---|---|
| **T0 spike** (~3–4 days, throwaway branch) | napi-rs addon on `radicle =0.25.1`. **Workflow 1 end to end** (`Profile.init` → `initProject` → `head()`/`identity()` → `forkProject` → `checkoutProject`), plus a `commit()` read to exercise byte fields and issue create/comment, as Promises on Node 26.12.0; `Profile.load()`/`signer()` are synchronous setup exceptions. Comment result envelopes and detached snapshot/byte-copy tests. Signer crossing to worker tasks. §5 lock contract and tests. Timestamp boundary tests (upstream max values, overflow, out-of-`Date`-range rejection). Check generated `.d.ts` against §4 (literal unions, readonly, Promise signatures, `Uint8Array`) and runtime behaviour (frozen object/array containers, mutable independent byte copies, `RadicleError` instances/codes, `TypeError`/`RangeError`). **Alpha install command:** from a clean project, `npm install "git+${SDK_GIT_URL}#<commit>"` (the seed URL form the Python README uses) builds and loads the addon; pnpm (build-script opt-in documented) and Bun best effort | All of the listed operations pass on Linux. A FINDINGS file records the reproducible Send/Sync assertions, concurrency results, generated types and installation results |
| **Decision** | Using T0 evidence: which operations were reusable as-is, what extraction would cost radicle-py users, mono-repo or not | Written decision. T1 runs only if it says so |
| **T1 core extraction** (conditional, ~3–4 days) | `radicle-sdk-core`, Python layer becomes thin conversions | §6 gates all pass; no Python API, stub or API.md changes |
| **T2 TypeScript v0** (~1–1½ weeks) | §2 surface; documentation of `git push rad` vs. SDK `create()`; README quick start executed by a test | The three workflows pass; error, validation and concurrency tests pass |

**Conformance:** reuse the three workflow expectations as small, separate tests
in each language. Add a shared scenario framework only if keeping those tests
in step proves to be a burden.

### Alpha distribution

Distribution mirrors radicle-py: build from source via Git; npm publication and
prebuilt packages are deferred until there are actual users.

- **A `package.json` at the repository root**, next to `pyproject.toml`. npm's
  Git dependencies can't select a subdirectory (only `#<commit-ish>` or
  `#semver:`), so a root manifest is what makes the one-line install work with
  npm, pnpm and Bun. Its `prepare` script runs the napi-rs build with
  `--manifest-path` pointing at the Rust crate, wherever it lives, so this
  doesn't constrain the Rust layout or the mono-repo decision.
- **Install command for alpha users**, the same shape as pip's:
  `npm install "git+${SDK_GIT_URL}#<commit>"`. It needs Rust and a C
  toolchain. Minimum Rust is 1.91 (measured; see above).
- **Package-manager evidence:** npm and pnpm root Git installs passed in T0.
  pnpm 12 requires exact Git-source approval in `allowBuilds`; see `node/README.md`.
  Bun Git prepare did not install the build tools and failed; it remains best
  effort, despite a successful runtime test with the Node-built addon.
- **Fallback, documented but not tested in CI:** clone, build once in the
  package directory, then `npm install /path/to/checkout/<package-dir>`.
- When npm publication happens, the package can be published from any
  directory; the root manifest can stay or move.

## 8. Decisions deferred until evidence

No remaining pre-T0 blockers. The working name is `radicle-sdk` and the supported
Node major is 26 (Active LTS scheduled for 2026-10-28), initially targeted at
26.12.0. npm name reservation,
publication, additional supported runtimes, shared-core extraction and a JS
write queue are deferred—not tasks to complete before the spike.

Keep explicit `base` and upstream empty-patch acceptance for programmatic
`create()`/`draft()` in v0. Prefer Git for working-copy flows as documented in
§1. Revisit defaults only if actual use demonstrates a need; no corresponding
Python API change is proposed.

## Sources

- Selected Node target: [official release index](https://nodejs.org/dist/index.json), checked 2026-10-15 (v26.12.0, currently non-LTS); [official release schedule](https://github.com/nodejs/Release/blob/main/schedule.json) (Node 26 Active LTS: 2026-10-28); [Node release policy](https://nodejs.org/en/about/previous-releases)
- radicle-httpd role and read-only seed configuration: [Radicle Seeder Guide](https://radicle.dev/guides/seeder), [MyNixOS radicle-httpd](https://mynixos.com/nixpkgs/package/radicle-httpd)
- Crate churn (1.7.0 → radicle 0.21.0): [Radicle 1.7.0 release notes](https://radicle.dev/2026/03/18/radicle-1.7.0)
- napi-rs AsyncTask: [napi.rs AsyncTask](https://napi.rs/docs/concepts/async-task); v3 requirements: [v2→v3 migration](https://napi.rs/docs/more/v2-v3-migration-guide)
- Runtime support: [napi.rs](https://napi.rs/), [napi.rs support & compatibility](https://napi.rs/docs/more/support-compatibility), [Bun Node-API docs](https://bun.com/docs/api/node-api)
- rad-pi: [`@rad-pi/core` registry metadata](https://registry.npmjs.org/@rad-pi/core/latest), [`extensions/radicle-tools.ts`](https://unpkg.com/@rad-pi/core@1.6.1/extensions/radicle-tools.ts)
- Existing npm packages: [jsradicle](https://npmjs.com/package/jsradicle)
- npm Git dependencies (fragments limited to commit-ish/`semver:`; `prepare` builds): [npm package.json docs](https://docs.npmjs.com/cli/v11/configuring-npm/package-json#git-urls-as-dependencies); pnpm `#path:` subdirectories: [pnpm package sources](https://pnpm.io/package-sources)
- Send/Sync: local compile check against the pinned crate (to be reproduced in T0).
