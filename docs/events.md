# Events: primitive contract, change records and cursor

Status: **draft contract, not a shipped API.** This page turns the direction
recorded on events issue `089606916a4c782f0eb210cc9d8d5d3bb46ace33` (comments
`bb370ff`, `36521a3`, `af7b3f8`) into the rules both bindings are implemented
and tested against. It specifies two read-only primitives, how a poll derives
reconciliation records, and the cursor text. It does not yet fix the public
`since`/`watch` API; those wrappers follow the retention experiments below.
Operation delivery is retained only in the deferred-design appendix. The prototype that motivated it is described in
[the events experiment](../examples/events_experiment.md); where the two
differ, this page wins. The current application implementation and real-storage
results are in [the reconciliation experiment](../examples/reconciliation.md).

The anchor is Git: where the model has a choice, it takes the shape `git fetch`
already gives. Scope is one repository per feed. A
consumer that follows several repositories loops over `storage.repositories()`
with one cursor each.

## Initial scope: signed-ref reconciliation

Primitive 3 (bounded causal walk) and operation delivery are **deferred**. The
first implementation adds primitives 1 and 2 in both bindings, then rebuilds
the experiment as a signed-ref reconciliation feed. It identifies affected
COBs for consumers to reload through existing typed APIs; it does not promise
every intermediate action, operation ordering, or operation-ID deduplication.

The initial COB records are `created`, `changed`, and `deleted`, carrying old
and new union-of-namespace tip sets. `changed` means only that two non-empty
sets differ: there is no causal fast-forward check and no COB `forced` flag.
Namespace records and history-unavailable handling remain as specified below,
including incomplete old-frontier markers. A record computed from incomplete
old sets is not proof of creation. Delivery is at least once; consumers handle
a whole page before checkpointing and reconcile affected COB snapshots.

The operation-range primitive, operation events, and the `updated`/`forced`
COB distinction in the appendix are retained as **deferred design**, not initial delivery
requirements. No watcher API is added with either primitive. The experiment
will establish the smaller feed's usefulness and cost before public wrappers.

## Terms

- **Namespace**: one remote's refs under `refs/namespaces/<nid>/`, where
  `<nid>` is a Node ID.
- **Sigrefs commit**: the commit at `refs/namespaces/<nid>/refs/rad/sigrefs`.
  Its tree holds a `refs` blob (the signed ref set) and a `signature` blob.
  Each sigrefs commit records its parent; a namespace's sigrefs history is a
  chain, exposed as `parent` on the existing `SignedRefs` value.
- **Signed ref set**: the refname-to-OID map inside a sigrefs commit, keyed by
  names relative to the namespace (`refs/cobs/...`, `refs/heads/...`,
  `refs/rad/...`).
- **COB ref**: a signed ref named `refs/cobs/<typename>/<id>`. Its target is
  the change commit at that namespace's tip of the COB. Entries under
  `refs/rad/` and `refs/heads/` are not COB refs and are ignored by the feed;
  the identity document under `refs/rad/id` is out of scope.
- **Tip set / frontier**: for one COB, the set of distinct COB ref targets
  across all namespaces. One COB tracked from several namespaces is one branch
  tracked from several remotes; the union is the frontier.
- **Cursor**: the per-namespace sigrefs tips a consumer last observed, as text.

## Observation contract

- **Per repository.** A feed is opened on one repository and reports changes in
  that repository only. Storage-wide feeds are an application loop, not an SDK
  concept.
- **Signed changes only.** The feed observes sigrefs tips. A write into storage
  that is not followed by a signature is invisible until the next signature from
  that namespace. Re-signing an unchanged set reuses the sigrefs commit and does
  not advertise an earlier unsigned write; the next signed write advertises
  the resulting tip set, triggering reconciliation rather than action replay.
  Every SDK and CLI COB write signs, so in practice this affects only direct
  `git push` into storage. Validated by the experiment; documented, not fixed.
- **What is verified.** The feed verifies nothing itself. Signed ref sets are
  loaded through upstream's `SignedRefs::load_at`, whose reader verifies the
  sigrefs commit's signature against the namespace key, checks its binding to
  the repository identity and applies upstream's replay protection; the feed
  relies on that and claims nothing beyond it. It does not load or verify
  operation signatures or authorization, and a changed tip is not proof of an
  authorized or applied change. (Precision over `af7b3f8`, which said "does not
  verify signatures": the feed does not, upstream's loader does.)
- **At least once.** Consumers reconcile affected COBs idempotently and
  checkpoint the new cursor only after a whole page was handled. Consumers own
  cursor storage. No exactly-once, no acknowledgement, no durable journal.

## Primitives

Two read-only operations, implemented natively in both bindings with the same
semantics and cross-SDK fixtures. Both live on `Repository` beside `remotes()`
and `sign_refs`/`signRefs`. They are building blocks, not a watcher API.

### 1. Namespace tips

```text
namespace_tips(repository) -> [(nid, oid)]   sorted by nid, bytewise
```

Lists `refs/namespaces/*/refs/rad/sigrefs` with each ref's target OID. This is
the ref-tip comparison a poll is built on. Public names are
`Repository.namespace_tips()` (Python) and `Repository.namespaceTips()`
(TypeScript), returning immutable `NamespaceTip` values (`node_id`/`nodeId`,
`target`).

- Reads refs only: no sigrefs commit, tree or blob is loaded and no signed ref
  name is materialised. Upstream's `Repository::remote_ids` already globs
  without loading; this primitive is that glob plus the target. `remotes()`
  stays as it is (it loads every signed set) and is not used by the feed.
- The target is the ref's resolved OID (a symbolic ref is followed). Whether
  that OID is a loadable sigrefs commit is checked only when its set is loaded
  (primitive 2); a ref that cannot be resolved is a Git error.
- A namespace component that does not parse as a Node ID is a Git error, as in
  upstream `remote_ids` (`InvalidRef`). This differs from `Raw.typenames()`,
  which skips lookalike COB refs: a malformed sigrefs ref cannot be skipped
  without silently dropping a namespace from every cursor.
- Cost: O(namespaces) ref reads. Idle polls do nothing else.
- Errors: Python `GitError`, Node code `GIT`.

### 2. Signed ref set at a sigrefs commit

```text
signed_refs_at(repository, nid, oid) -> SignedRefs
```

Loads the signed ref set recorded at sigrefs commit `oid` for namespace `nid`,
through upstream `SignedRefs::load_at`. The result is the existing `SignedRefs`
value (`node_id`, `at`, `parent`, `feature_level`, `refs()`), so no new type.
Public names are `Repository.signed_refs_at(node_id, oid)` (Python) and
`Repository.signedRefsAt(nodeId, oid)` (TypeScript; `refs` is a frozen record).

- The `refs` blob is never parsed by the SDKs; its format is an upstream
  internal.
- `at` is what upstream returns, which for pre-`parent` feature levels can be
  an ancestor of the requested commit when upstream's replay protection skips
  duplicated signatures. The feed compares and records the ref target it read
  (the requested `oid`); the ref set it uses is the one upstream yields for it.
  Upstream also promotes a verified root commit at feature level `root` to
  `parent` on read; metadata therefore need not equal the original signing
  result's feature level. Neither behavior is normalized by the adapters.
- Upstream returns "no sigrefs" only for a reference lookup; for an explicit
  commit a missing object is a read error. Every upstream read or verification
  failure maps to a Git error carrying upstream's message. The feed classifies
  that error by which tip it was loading (see history-unavailable below); the
  primitive itself does not.
- Cost: at the current `parent` feature level, one commit, one tree and two
  blobs per call plus one signature verification. At older feature levels
  upstream's reader walks the sigrefs history to detect replayed signatures
  and may make more than one pass, so the cost is history-dependent; this is
  upstream's contract, not a fast path the feed can assume. Parsing is
  O(refs in that namespace), which is inherent to the design: a signed set is
  one blob.
- Errors: Python `GitError`, Node code `GIT`; a `nid` or `oid` that does not
  parse is an argument error.

## A poll: from two cursors to change records

`since(cursor)` performs every read before returning anything. On any error the
page is not returned and the cursor is unchanged; there is no partial page.

1. Parse the cursor strictly (grammar below). A cursor for another repository
   is an argument error.
2. Read the current namespace tips (primitive 1). These exact OIDs become the
   new cursor, so a namespace that moves again during the poll is seen on the
   next one, never half-read.
3. If no tip differs from the cursor, return an empty page with the same
   cursor. Nothing else is loaded.
4. Load signed ref sets (primitive 2): the old set of every cursor namespace
   whose tip changed or disappeared, the new set of every namespace whose tip
   changed or is new, and the set of every unchanged namespace at its unchanged
   tip, because COB frontiers are unions across all namespaces. Cost:
   O(namespaces) object reads, no ref listing. An implementation may cache sets
   by sigrefs OID within a process; sigrefs commits are immutable.
5. Derive namespace records, then COB records (`created`, `changed`, `deleted`)
   by comparing aggregate tip sets. Do not walk operation history.

### Namespace records

One per namespace whose tip differs between the cursors, shaped like the node's
`RefUpdate { old, new }`:

| Record | Condition |
| --- | --- |
| `created` | not in the cursor, present now |
| `updated` | both known; the old sigrefs commit is a Git ancestor of the new one |
| `forced` | both known and readable; the ancestry walk completed and the old commit is not an ancestor |
| `removed` | in the cursor, no sigrefs ref now |
| `history-unavailable` | in the cursor, whether or not still present, but the old sigrefs commit or its ref set cannot be loaded (missing or pruned object, verification failure, any upstream read error); carries upstream's message |

Ancestry is plain Git commit ancestry between the two sigrefs commits, which
is what `git fetch` checks for a remote-tracking ref and what upstream's
`refs/rad/sigrefs-parent` entry is verified against; it loads no ref sets.
`forced` is a proven divergence. A walk that cannot load a commit on the new
tip's chain proves neither ancestry nor divergence; those commits are
reachable from the current tip, so their absence means the present state is
unreadable and the poll fails with a Git error, like an unreadable current
tip below. The flag is context for the consumer; COB records below do not
depend on it. Namespace-level `forced` deliberately remains in initial scope:
it is plain Git ancestry between sigrefs commits, loads no signed sets, and is
the reconciliation feed's only explicit backward-movement signal.

`history-unavailable` is its own condition, not a forced update: without the
old ref set the feed does not know which COBs that namespace advertised and
must not invent per-COB records. Such a namespace contributes nothing to the
old frontiers in the step below (its current set, if any, still contributes to
the new ones). The page then lists it under `incomplete`, and every COB record
in that page carries `old_complete: false`: its `old_tips` are the union over
the readable old sets only, not a proven previous frontier, and a `created`
record in such a page means "not advertised by any readable old set", not a
proven creation. Re-delivery of a COB the unavailable namespace alone
advertised is covered by at-least-once delivery. The consumer resolves the
condition as it would a forced update: resync that namespace from its current
tip (its current signed set is in the page's new frontiers). The new cursor
already carries the current tip, so the page, not the cursor, is what tells
the consumer a gap exists; a consumer that checkpoints without reading the
records has chosen to skip it.

A namespace whose **current** tip cannot be loaded is a poll error, not a
record: the repository's present state is unreadable and there is nothing
safe to advance to.

### COB records

A signed name with `refs/cobs/<typename>/<id>` shape whose object ID does not
parse is a storage condition: fail the poll with Python `GitError` / Node `GIT`,
not an argument error or a silently skipped record. This applies to either old
or new signed sets when deriving frontiers.

For every COB ref name in the union of all old and new signed sets, let
`old_tips` be the union of its targets across the old sets and `new_tips` the
union across the new sets (unchanged namespaces contribute the same set to
both).

| Record | Condition |
| --- | --- |
| `created` | `old_tips` empty, `new_tips` not |
| `changed` | both non-empty and different |
| `deleted` | `new_tips` empty, `old_tips` not |
| none | `old_tips == new_tips` |

`changed` is a trigger, not a diff: the aggregate tip set differs, but a reload
may find no visible difference (for example a namespace dropping a redundant
tip). A rewind is reported as `changed`, not distinguished. No causal ancestry
check or operation history load is required. A COB is `deleted` only when it
is absent from every current namespace's signed set.

Every record carries the COB key (`typename`, `id`), change kind, `old_tips`,
`new_tips` (both sorted) and `old_complete` (false only in a page with a
history-unavailable namespace). The old tips and `created` classification are
provisional when `old_complete` is false. Records carry no operation list or
causal flag.

### Page shape and order

```text
Page {
  rid
  cursor          // the cursor this page was computed from
  next            // the cursor to checkpoint after handling the page
  namespaces      // namespace records, sorted by nid
  incomplete      // nids whose old set was unavailable (usually empty)
  cobs            // COB reconciliation records, sorted by (typename, id)
}
```

Pages are deterministic for the same storage and cursor, in both bindings.
There is no order across COBs beyond the sort and no operation or timestamp
ordering promise.

## Cursor text

The cursor is the namespace tip listing in the `<oid> <refname>` form Git uses
for `packed-refs` and `ls-remote`, under a header that binds it to one
repository and one format version.

```text
# radicle-events v1 rad:z27dDy6sppkZDG8qYcySVX9Qn5jNg
3f2c0b6e9e4a7d1c8b5a0f3e6d9c2b1a4e7f0d3c refs/namespaces/z6MkhaXgBZDvotDkL5257faiztiGiC2QtKLGpbnnEGta2doK/refs/rad/sigrefs
9b1e4d7a0c3f6e9b2d5a8c1f4e7b0d3a6c9f2e5b refs/namespaces/z6MkltRpSYTxsmQ9f4xEcX7dn3GWjXr6mY6Dcr6JsUNqUTSz/refs/rad/sigrefs
```

Grammar (ABNF, `LF` is a single 0x0A):

```text
cursor  = header *line
header  = "# radicle-events v1 " rid LF
line    = oid SP refname LF
oid     = 40HEXDIG                       ; lowercase, not all zeros
refname = "refs/namespaces/" nid "/refs/rad/sigrefs"
rid     = "rad:" <RepoId as printed by the SDKs>
nid     = <NodeId as printed by the SDKs>
```

Rules:

- **Canonical.** Lines sorted by refname bytewise, no duplicate refname, LF
  line endings only, no trailing whitespace, no blank lines, no other lines, no
  CR, no BOM. A cursor with a header and no lines is valid and means "nothing
  observed": `since` from it replays every namespace and every COB as
  `created`. To start without a replay, take the current tips as a cursor
  first (the `git ls-remote` move); that is the baseline the observers used.
- **Strict.** A parser rejects, with an argument error naming the first
  offending line: a wrong or missing header, any version other than `v1`, a
  RID that is not this repository's, an unsorted or duplicated refname, an
  uppercase, short, long or all-zero OID, a refname outside the pattern, a
  `nid` or `rid` that does not parse, a missing final LF, or any trailing
  content. Parsers never normalise: both SDKs emit canonical text, so a
  consumer can compare cursors bytewise and diff them with ordinary tools.
- **Byte-identical across SDKs.** For the same tips Python and TypeScript
  produce the same bytes; a cursor written by one is read by the other. This
  is a cross-SDK test.
- **Readable, not hand-written.** Consumers may read and diff it; they should
  not construct one from values the feed did not return, since a tip that
  upstream cannot load at the next poll surfaces as `history-unavailable`.
- Size: about one hundred bytes per namespace. Only `v1` exists; a semantic
  change to any rule on this page bumps the version.

## Retention and the experiments that gate the API

Run namespace retention, cursor handoff, unsigned visibility and aggregate-tip
reconciliation cases with primitives 1 and 2. A missing current signed-set object
fails the poll. The full scan remains an oracle for signed frontier changes,
not a claim of operation delivery.

An old cursor is resolvable while its sigrefs commits are loadable. The parent
chain explains the common case: every signed write produces a child commit, so
an old tip stays reachable from the current one until that namespace is
force-updated, after which the old commit may be pruned. The answer is Git's:
resync from the current tip via the record above. There is no retention policy
to add, only this rule to document and verify.

Before any public API, the experiment is rebuilt on the two primitives, kept
against the full scan as oracle, and run on the heartwood storage on the
development host. Each case below must produce the stated outcome, never an
empty page or a cursor that quietly moved past it:

| Case | Expected |
| --- | --- |
| Namespace removed (`refs/namespaces/<nid>/` deleted, cursor still names it) | namespace `removed`; COB `deleted` only for COBs no other namespace advertises |
| Namespace force-updated so the cursor's sigrefs commit is no longer an ancestor; commit still present | namespace `forced`; COB records computed normally from both sets |
| Same, then `git gc --prune=now` removes the orphaned commit | namespace `history-unavailable` carrying upstream's message, reported in the page; no per-COB records for that namespace |
| Current signed set or a required ancestry object unreadable | poll error (Git), cursor unchanged |
| Unsigned COB write, then re-sign of the unchanged set, then a signed write | first two polls empty; third reports the changed signed tip set for reconciliation (scan oracle differs, by contract) |
| Two namespaces advertise one COB, one drops its ref | no record if the union is unchanged; `changed` if it narrows; `deleted` only once both drop it |
| Cross-SDK: cursor written by one SDK, next page computed by the other | identical page, byte-identical `next` |

Results, including per-poll timings against the scan oracle, are recorded on
issue `0896069`. The public `since`/`watch` wrappers sketched there are
specified after that record exists.

## Test obligations for the primitive patches

- Primitive 1 lists the same namespaces as `remotes()` and loads nothing: a
  fixture whose sigrefs ref targets a missing commit still lists that namespace
  with that OID, while `remotes()` fails on it.
- Primitive 2 at the current tip equals `remotes()[nid].refs`; at an older tip
  equals the set recorded by the `SignedRefs` value returned when that commit
  was created; a pruned commit is a Git error naming the OID.
- Cursor: a property test that parse(emit(tips)) round-trips, and a table of
  rejected inputs covering every bullet under "Strict".
- Every fixture is shared with the Node tests through the existing cross-SDK
  harness (`RADICLE_REQUIRE_CROSS_SDK=1`).

## Decisions

Settled in the review of this page (patch `756f2de`):

- Initial implementation is primitives 1 and 2 plus a reconciliation experiment;
  primitive 3 and operation delivery are deferred to demonstrated consumer need.

- Primitives 1 and 2 live on `Repository`. Final names per language are chosen
  in the implementation patches, following each SDK's existing conventions.
- `signed_refs_at` returns upstream's `SignedRefs` unchanged; `at` keeps
  upstream's meaning and the caller keeps the OID it asked for. The feed
  records the ref target it read.
- Replay from an empty cursor is feed behaviour. A silent baseline is the
  consumer taking the current tips as its first cursor, explicitly.

## Appendix: deferred operation-feed design

This appendix preserves the reviewed causal design for a future demonstrated
consumer need. It is not part of the initial reconciliation implementation.
Primitive 3 would live on `Raw` beside `history()`.

An **operation** is one change commit (an upstream `cob::change::Entry`),
identified by its OID, with causal `parents` within the same COB. It is surfaced
today as `RawOp` by `Raw.history()`.

### Operation delivery

One event per operation, with every action and its index,
`evaluation: not-evaluated`. Evaluated or "applied" views sit above the feed;
the feed never filters an operation because upstream would ignore it.
Operation signatures and authorization are parsed to find the actor and are
**not verified**: an operation being delivered does not mean it was authorized
or applied. Consumers deduplicate on operation OID.

### 3. Bounded causal walk (deferred)

```text
operations_between(raw, typename, id, old_tips, new_tips)
  -> { fast_forward: bool, operations: [RawOp] }
```

The operations in the revision range `^old_tips new_tips` for one COB:
reachable from any new tip minus reachable from any old tip, where
reachability follows causal `parents` only (the same edge `Raw.history()`
follows; `related` and the identity resource link are never traversed).

- Semantics are set difference: with `reach(T)` the set of operations
  reachable from any tip in `T` (including the tips), the result is
  `reach(new_tips) \ reach(old_tips)` and `fast_forward` is
  `old_tips ⊆ reach(new_tips)`. Both are defined on sets, so an old tip that
  is an ancestor of another old tip changes nothing, and an old tip's
  ancestors reached through a different branch are still excluded.
- `fast_forward` is a flag, not an error: the range is well defined across
  divergent histories and is always returned. Suppressing operations on a
  non-fast-forward is a consumer policy, not a feed rule.
- Order: parents before children; among operations whose parents have all been
  emitted, the lexicographically smallest OID first. This is the order
  `Raw.history()` already guarantees.
- The algorithm is not prescribed. Stopping at old tips is not sufficient:
  with `R -> A`, `R -> B` and a merge `M` of `A` and `B`, old `{A}` and new
  `{M}`, a walk that stops at `A` still reaches `R` through `B`, yet the range
  is `{B, M}`; and with `A -> B -> C`, old `{A, B}` and new `{C}`, stopping
  at `B` never meets `A` although the update is a fast-forward. An
  implementation loads at least the entries in the range and at most
  `reach(new_tips) ∪ reach(old_tips)`; a rewind may cost the COB's full
  history, which is acceptable.
- `old_tips` may be empty: the result is the full history from the new tips and
  `fast_forward` is true. `new_tips` must be non-empty. Tips are deduplicated;
  an old tip that is also a new tip contributes nothing to the range.
- A parent OID that cannot be loaded is a COB error naming the entry, not a
  shorter range: a missing object must never produce a partial page. A causal
  cycle is a COB error, as today.
- Invariant, tested: for any COB, `operations_between(t, id, [], current ref
  targets)` returns the same list as `Raw.history(t, id)`.
- Errors: Python `CobError`, Node code `COB`; malformed typename, id or OIDs
  are argument errors.

### Causal COB records

For every COB ref name in the union of all old and new signed sets, let
`old_tips` be the union of its targets across the old sets and `new_tips` the
union across the new sets (unchanged namespaces contribute the same set to
both).

| Record | Condition | Operations |
| --- | --- | --- |
| `created` | `old_tips` empty, `new_tips` not | full history from `new_tips` |
| `updated` | both non-empty, different, every old tip reachable from the union of `new_tips` | `^old new` |
| `forced` | both non-empty, different, some old tip not reachable from the union | `^old new` (still emitted) |
| `deleted` | `new_tips` empty, `old_tips` not | none; carries the last known tips |
| none | `old_tips == new_tips` | none |

Reachability is "from the union of the new tips", not from each new tip. One
namespace dropping its ref only narrows the union: if another namespace still
advertises a tip from which the old tips are reachable, the record is at most
`updated`, and it is `deleted` only when the COB is absent from every signed
set. A narrowing that leaves some old tip unreachable is `forced`.

Every record carries the COB key (`typename`, `id`), `old_tips`, `new_tips`
(both sorted), the flag and `old_complete` (false only in a page with a
history-unavailable namespace, see above). `forced` differs from `updated`
only by the flag; the operation list is the same range. What to do with
operations under a forced flag is the consumer's choice, as after `git fetch`
prints a forced update.

### Operation-page ordering

The deferred page adds operations in walk order to each COB record. There is
no order across COBs beyond the sort, and no timestamp order anywhere:
`timestamp` is author-claimed.

### Deferred retention and test obligations

| Case | Expected |
| --- | --- |
| Object missing inside an otherwise reachable operation range | poll error (COB), cursor unchanged |
| Unsigned COB write, then re-sign of the unchanged set, then a signed write | first two polls empty; third delivers both operations (scan oracle differs, by contract) |
| Two namespaces advertise one COB, one drops its ref | no record if the union is unchanged; `updated` if the dropped tip is reachable from the remaining union; `forced` if it is not; `deleted` only once both drop it |

Primitive 3 reproduces `Raw.history()` with empty old tips; returns exactly
the delta for a fast-forward; returns the divergent range with
`fast_forward: false` after a rewind; raises on a missing parent; and
passes the two DAG cases above: the merge whose other branch reaches an old
tip's ancestors (`{B, M}`, not `R`), and redundant old tips (`{A, B}` under
`C` is a fast-forward returning `{C}`). These fixtures would use the same
cross-SDK harness as primitives 1 and 2.
