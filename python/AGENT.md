# `radiclepy` agent quick reference

Use this page to route a Radicle automation task. See [API.md](API.md) for exact
signatures and [workflow semantics](../docs/semantics.md) for edge cases.

## Scope

`radiclepy` performs **local** project, repository, issue, patch, policy, routing,
and raw COB operations. It does not fetch, announce, connect to peers, or control
a node. Git owns working-copy commits, branches, diffs, merges, and commit
transport.

## Object model

```text
Profile
├── storage -> Storage -> Repository
├── signer() -> Signer                 # acquire only for writes
├── policy_store() -> PolicyStore
└── routing_store() -> RoutingStore

Repository -> Issues / Patches / Raw
```

`Issues.open(repo)` and `Patches.open(repo)` are read-only.
`open_as(repo, signer)` enables signed local mutations. Snapshots are detached;
call `reload()` when current state matters.

## Task map

| Intent | Start here |
| --- | --- |
| Find the project for a working copy | `project_at(path)` |
| List locally stored projects | `Profile.load().storage.repositories()` |
| Inspect identity, Git objects, refs, or history | `storage.repository(rid)` |
| Read issues or patches | `Issues.open(repo)`, `Patches.open(repo)` |
| Create or update an issue | `Issues.open_as(repo, signer)` |
| Review or comment on a patch | `Patches.open_as(repo, signer).get_mut(id)` |
| Inspect an unknown COB type | `Raw.open(repo)` |
| Change local follow/seed policy | `profile.policy_store()` |
| Read local routing information | `profile.routing_store()` |
| Initialize a committed Git repository | `init_project(...)` |
| Checkout a project already in storage | `checkout_project(...)` |
| Fetch, announce, sync, or control a node | Not supported; use `rad` |
| Create commits, diffs, branches, or Git merges | Use Git |
| Open/update a patch from a working copy | Push with `git-remote-rad` |

## Existing working copy

Preconditions: an existing profile, a working copy with a `rad` fetch remote,
and the project present in local storage.

```python
import radiclepy as rad

profile = rad.Profile.load()
location = rad.project_at(".")
if location is None:
    raise RuntimeError("not a Radicle working copy")
repo = profile.storage.repository(location.rid)

for issue in rad.Issues.open(repo).all():
    print(issue.id, issue.state, issue.title)
```

Browsing does not require a signer.

## Signed local writes

```python
signer = profile.signer()
issues = rad.Issues.open_as(repo, signer)
issue = issues.create(
    "Automation report",
    "Created through radiclepy",
    labels=["automation"],
)
issue.comment("Additional context")
observed = issue.reload()
```

This signs locally but does **not** announce. Use `rad sync --announce`
separately when announcement is intended.

For patch review, prefer a user-supplied revision ID. Otherwise use the patch
author's latest revision and inspect the result afterward:

```python
patch_mut = rad.Patches.open_as(repo, signer).get_mut(patch_id)
revision_id = patch_mut.reload().latest_revision
patch_mut.review(revision_id, verdict="accept", summary="Looks good")
observed = patch_mut.reload()
```

Upstream retains the first review by an actor on a revision; a later recorded
review may not replace it.

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
- Reload and inspect state after reviews, lifecycle changes, and merge records.
- `label()` and `assign()` replace the **entire** set. Serialize the complete
  read/compute/write/result-check sequence for each storage path and RID.
- A local lock does not coordinate other processes, CLI writers, or independent
  application instances.
- `PatchMut.merge()` records a COB merge operation; it does not perform a Git
  merge or move a branch.
- Do not blindly retry after an exception. Persistence may already have occurred;
  inspect state first.
- Mutable stores, mutable handles, and signers are thread-affine.
- Typed collaboration timestamps are milliseconds; raw COB timestamps are
  seconds.

## Errors

Invalid caller input generally raises `ValueError` (or Python conversion
`TypeError`/`OverflowError`). Operational failures inherit from
`radiclepy.Error`. Missing `Issues.get()` / `Patches.get()` returns `None`;
listing failures do not become empty successful lists. There is no automatic
retry, rollback, or multi-call transaction.

## Details

- [Python guide](README.md)
- [Complete Python API](API.md)
- [Shared workflow semantics](../docs/semantics.md)
- [Python/TypeScript differences](../docs/sdk-differences.md)
- [Tested examples](../examples/README.md)
