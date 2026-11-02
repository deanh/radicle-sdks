# `radicle-sdk` agent quick reference

Use this page to route a Radicle automation task. See [API.md](API.md) for exact
types and [workflow semantics](../docs/semantics.md) for edge cases.

## Scope

`radicle-sdk` performs **local** project, repository, issue, patch, policy,
routing, and raw COB operations. It does not fetch, announce, connect to peers,
or control a node. Git owns working-copy commits, branches, diffs, merges, and
commit transport.

## Object model

```text
Profile
├── storage -> Storage -> Repository
├── signerAsync() -> Signer             # acquire only for writes
├── policyStore() -> PolicyStore
└── routingStore() -> RoutingStore

Repository -> Issues / Patches / Raw
```

`Issues.open(repo)` and `Patches.open(repo)` are read-only.
`openAs(repo, signer)` enables signed local mutations. Snapshots are detached and
frozen; copied `Uint8Array` fields remain mutable.

## Task map

| Intent | Start here |
| --- | --- |
| Find the project for a working copy | `await projectAt(path)` |
| List locally stored projects | `await profile.storage.repositories()` |
| Inspect identity, Git objects, refs, or history | `profile.storage.repository(rid)` |
| Read issues or patches | `Issues.open(repo)`, `Patches.open(repo)` |
| Create or update an issue | `Issues.openAs(repo, signer)` |
| Review or comment on a patch | `await Patches.openAs(repo, signer).getMut(id)` |
| Inspect an unknown COB type | `Raw.open(repo)` |
| Change local follow/seed policy | `await profile.policyStore()` |
| Read local routing information | `await profile.routingStore()` |
| Initialize a committed Git repository | `await initProject(...)` |
| Checkout a project already in storage | `await checkoutProject(...)` |
| Fetch, announce, sync, or control a node | Not supported; use `rad` |
| Create commits, diffs, branches, or Git merges | Use Git |
| Open/update a patch from a working copy | Push with `git-remote-rad` |

## Async model

- Prefer `Profile.loadAsync()` and `signerAsync()` in interactive hosts.
- `Profile.load()` and `profile.signer()` are synchronous and may block.
- Handle factories/getters are often synchronous; repository/store operations
  return Promises and must be awaited.
- Stopping a Promise wait does not cancel or roll back native work.

## Existing working copy

Preconditions: an existing profile, a working copy with a `rad` fetch remote,
and the project present in local storage.

```ts
import { Issues, Patches, Profile, projectAt } from 'radicle-sdk';

const [profile, location] = await Promise.all([
  Profile.loadAsync(),
  projectAt('.'),
]);
if (location === null) throw new Error('not a Radicle working copy');
const repo = profile.storage.repository(location.rid);

for (const issue of await Issues.open(repo).all()) {
  console.log(issue.id, issue.state, issue.title);
}
```

Browsing does not require a signer.

## Signed local writes

```ts
const signer = await profile.signerAsync();
const issue = await Issues.openAs(repo, signer).create({
  title: 'Automation report',
  description: 'Created through radicle-sdk',
  labels: ['automation'],
});
const result = await issue.comment('Additional context');
console.log(result.entryId, result.snapshot);
```

This signs locally but does **not** announce. Use `rad sync --announce`
separately when announcement is intended. Creation returns a mutable handle;
later mutations return envelopes containing the observed `snapshot`.

For patch review, prefer a user-supplied revision ID. Otherwise use the patch
author's latest revision:

```ts
const patchMut = await Patches.openAs(repo, signer).getMut(patchId);
const revisionId = (await patchMut.reload()).latestRevision;
const result = await patchMut.review(revisionId, {
  verdict: 'accept', summary: 'Looks good',
});
console.log(result.entryId, result.snapshot);
```

Upstream retains the first review by an actor on a revision; a later recorded
review may not replace it. Inspect the selected revision in `result.snapshot`.

## SDK or Git for patches?

SDK `Patches.create/draft/update` is for `base` and `head` commits that already
exist in local storage. It does not import commits, calculate a merge base, move
a patch branch, or sync.

For a normal working-copy patch, use Git:

```sh
git push rad HEAD:refs/patches
git push rad HEAD:refs/heads/patches/<patch-id>
```

## Mutation safety

- Local signing is not network announcement.
- Inspect `result.snapshot`; `entryId` means recorded, not necessarily applied.
- `label()` and `assign()` replace the **entire** set. Serialize the complete
  read/compute/write/result-check sequence for each storage path and RID.
- A local queue does not coordinate other processes, CLI writers, separately
  loaded addons, or independent application instances.
- `PatchMut.merge()` records a COB merge operation; it does not perform a Git
  merge or move a branch.
- Do not blindly retry after a rejection. Persistence may already have occurred;
  inspect state first.
- Raw COB timestamps and manifest versions are `bigint`; convert them explicitly
  before `JSON.stringify`.
- Set `RAD_HOME`, `RAD_PASSPHRASE`, and `SSH_AUTH_SOCK` before process startup.

## Errors

Invalid arguments use Node-style `TypeError`/`RangeError` codes. Operational
failures are `RadicleError` values with codes such as `GIT`, `COB`, `SIGNER`, or
`READ_ONLY`. Missing `Issues.get()` / `Patches.get()` returns `null`; listing
failures do not become empty successful arrays. There is no automatic retry,
rollback, or multi-call transaction.

## Details

- [TypeScript guide](README.md)
- [Complete TypeScript API](API.md)
- [Shared workflow semantics](../docs/semantics.md)
- [Python/TypeScript differences](../docs/sdk-differences.md)
- [Tested examples](../examples/README.md)
