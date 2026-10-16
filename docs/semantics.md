# Shared local workflow semantics

These rules describe behavior shared by the Python and TypeScript alphas over
Radicle **`=0.25.1`**. They do not imply identical signatures or full upstream API
coverage. See the [Python API](../python/API.md), [TypeScript API](../node/API.md)
and [SDK differences](sdk-differences.md) for binding-specific contracts.

## Local projects and identities

- Profile initialization creates local keys/configuration/storage. Resolving a
  signer uses upstream keystore/passphrase/SSH-agent behavior; public setup is
  not a custom environment or credential adapter.
- Project initialization adapts the crate, not the interactive `rad init` CLI.
  Callers supply an existing Git repository with a committed default branch and
  explicit metadata. Invalid short branches or a missing commit fail rather
  than creating a project around an uncommitted branch.
- Initialization refuses an existing `rad` remote before upstream destructive
  cleanup, preserving its configuration and tracking refs. It is not a project
  metadata-update operation.
- Public/private visibility is identity/access metadata, **not encryption** or a
  network publication operation. Private init uses an empty explicit allowlist;
  advanced access administration is outside this SDK scope.
- Fork operates on a project already in local storage. Checkout reads canonical
  heads from local storage, not a chosen peer's branch or a network clone. Its
  remote argument selects the configured push namespace, not checkout content.
- Git is required for upstream init/checkout subprocesses. SDK local workflows
  need neither the `rad` CLI nor a running node. Later external pushes to the
  configured Radicle remote require `git-remote-rad` separately.

## Git import versus COB writes

Git owns commit creation, object import and branch movement. The SDKs do not
provide a generic Git write API or an implicit commit-import step.

SDK patch create/draft/update calls take explicit `base` and `head` commit OIDs
that must already exist in storage. They do not calculate a default merge-base,
maintain `refs/heads/patches/<id>`, deduplicate repeated base/head pairs or sync.
Empty `base == head` patches are accepted by the upstream API.

For working-copy creation/update, the remote helper is usually more appropriate:

```sh
# ASCII single-line push-option messages; suppress optional remote-helper sync.
git push rad HEAD:refs/patches -o no-sync -o 'patch.message=Change' -o 'patch.message=Description'
# PATCH_ID is an existing patch ID.
git push rad "HEAD:refs/heads/patches/$PATCH_ID" -o no-sync
```

These pushes import commits and maintain the patch branch. Both use the
merge-base with canonical head unless `patch.base` overrides it. Creation rejects
empty patches. Updating skips existing base/head pairs and can record an open
patch as merged when base equals head. Without `no-sync`, the helper attempts
sync only when seeding and the node is running; sync failure does not fail the
local push. This helper behavior is not an SDK networking API.

SDK revision base/head remain reachable as related COB Git parents even without
a patch branch. Importing via an actor's namespaced default branch also moves
that actor's project branch and can affect canonical-head/merge evaluation;
choose import refs deliberately.

## Signing and operation boundaries

Typed issue/patch mutations use upstream COB transactions and sign refs upstream.
Repository `sign_refs`/`signRefs` is an explicit operation for direct Git/ref
changes; it does not import objects, move canonical heads or fetch/announce.
Unchanged refs may reuse an existing sigrefs commit. Signing results represent
the returned upstream signed set, not a later ref reread; structural sigrefs
parent metadata is exposed separately.

Python additionally exposes unsigned canonical-ref maintenance methods. They do
not silently compose a write with signing; callers choose sequencing explicitly.

There is no atomic multi-call transaction. Error results are not rollback
receipts; inspect persisted state after a partial or internal failure before
continuing deliberately. No automatic retry is added.

## Issues and discussions

- Typed lists are eager and ID-sorted; genuine store/item failures propagate,
  never become an empty successful listing. Missing entity lookups return the
  language's missing-value sentinel.
- Labels and assignees are replacement sets, not additive edits; an empty set
  clears them. Assignees are DIDs, not node IDs.
- Issue descriptions are root-comment bodies. Editing the description preserves
  the root comment ID; new comments default to replies to that root.
- Reopening clears the close reason. Repeated issue lifecycle calls still use
  upstream transactions; no synthetic changed-state optimization is added.

## Patch revisions and reviews

Top-level description/base/head/latest-revision select the **patch author's**
latest revision, not the last revision by any author. Revision lists omit redacted
slots. `version` is the upstream slot count minus one, including redactions and
other authors: it is **not an index** into the revision list. Select by revision ID.

Upstream retains the first review per actor/revision. A later review records an
operation but does not replace that review; editing reviews is not exposed.
Operation IDs do not necessarily identify a newly added visible comment/review.

Patch lifecycle booleans report the upstream transaction decision, not a generic
before/after state comparison. False no-ops record no operation. Repeated
`archive()` can return true and record transactions even when already archived.

## Merge records are not Git merges

A patch merge call records a COB operation for an existing storage commit. It
does not merge Git, move branches or clean up refs. Upstream evaluates the
revision/commit against target-branch/delegate rules. A recorded operation can
leave the patch open; inspect the returned or reloaded snapshot's evaluated
state and merge fields rather than assuming successful recording means merged.

## Repository ordering and bounded history

Storage enumeration follows upstream order; canonical branch short names are
sorted. Tree entries retain canonical Git order, including directory sorting as
`name/`; preserve raw filename bytes and modes rather than re-sorting by decoded
text. Only a missing branch becomes the missing-value sentinel; other Git errors
propagate.

History defaults to 100 entries, clamps larger valid limits to 1000, and starts
at offset zero. Bindings validate their own integer input domains. The walk uses
libgit2's default order, not a promised chronological/topological sort. It checks
every consumed result, including skipped entries, and reads one extra result to
determine whether another page exists. Corruption before the requested page must
not masquerade as an empty final page.

## Snapshots and concurrency limits

Snapshots are detached views, not subscriptions. Collection access does not
silently refresh them. A listing is not a repository-wide atomic snapshot across
concurrent writers. Finish external Git/CLI writes before asking the SDK to read
or sign their results. Neither binding coordinates unrelated external writers.

Python thread affinity and cached mutable-handle snapshots differ from Node
worker locking and result envelopes; the [difference table](sdk-differences.md)
and language references define those boundaries. A shared source crate would
not itself provide cross-language or cross-process synchronization.
