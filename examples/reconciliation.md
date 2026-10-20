# Signed-ref reconciliation experiment

Research for events issue `089606916a4c782f0eb210cc9d8d5d3bb46ace33`, using
[the reconciliation contract](../docs/events.md). This replaces neither the
historical [operation experiment](events_experiment.md) nor the SDK API.
Operation ranges/delivery remain deferred. All code here is application code
using installed public exports; no watcher API, signer, replies or node connection.

## Run it

```sh
# One poll, external cursor, silent baseline on first run:
python examples/reconcile_refs.py --state /tmp/project.cursor rad:z...
node examples/reconcile-refs.mjs --state /tmp/project.cursor rad:z...
# Initial replay instead (only affects a missing checkpoint):
python examples/reconcile_refs.py --state /tmp/replay.cursor --replay rad:z...
```

Import Python `since(repository, cursor)` or TypeScript `since(repository,
cursor)`: both return a fully materialized page plus read counters, without
writing. `poll_once` / `pollOnce` is an application delivery adapter: callback
receives the whole non-empty page; only success acknowledges `next`. Python
callbacks must be synchronous, TypeScript callbacks are awaited. The CLIs emit
one JSON line per non-empty page and flush/await output before checkpointing.
An idle poll returns an empty page to the caller and prints nothing.

The text cursor is byte-identical across languages: repository/version header,
sorted namespace sigrefs ref names, lowercase OIDs, LF only, strict parsing.
It intentionally binds to a RID, not a local storage path, so consumers can
hand it between SDKs or copies of the same repository. Checkpoint files must
be outside Radicle home/storage, not symlinks, and have an existing parent.
This is single-writer application storage: temporary file, fsync, atomic
rename, no directory-fsync durability or hostile-filesystem sandbox claim.

Failures acknowledge nothing. Callback or checkpoint failures can repeat a page;
there is no exactly-once delivery, automatic retry or transactional application
side effect. Invalid state never resets silently. A silent baseline records
current tips without loading/verifying their signed sets; use an empty cursor
replay when a complete initial view is required.

## What the experiment establishes

- Namespace records are `created`, `updated`, `forced`, `removed`, or
  `history-unavailable`. Namespace ancestry is ordinary Git ancestry between
  sigrefs commits, not COB causal ancestry.
- COB records are **created / changed / deleted** with aggregate old/new tip
  sets, not individual actions. Three offline edits/comments can produce one
  trigger. A COB rewind is merely `changed`.
- `changed` is a trigger, not a visible diff. Dropping a redundant namespace tip
  changed the union while a typed issue reload still had identical comment IDs.
  Consumers must reconcile idempotently, not interpret a trigger as a message
  to send. Typed reloads read current storage, not a snapshot pinned to the
  page's signed frontier; they may see concurrent or unsigned newer state.
- Missing old sets produce explicit namespace gaps and a page-level `incomplete`
  list; all COB records then have `old_complete: false`. Apparent `created`
  records are not proven creations. An application must handle namespace gaps,
  including objects no longer advertised, rather than blindly processing the
  COB list or checkpointing. There is no hidden re-baseline.
- An unreadable current set, or a required commit missing inside its ancestry,
  is a poll error when that read is required. Idle tip equality deliberately
  performs no object-integrity check: latent damage with unchanged tips is not
  detected until a poll needs to load those objects. All reads finish before
  application delivery.
- Python/TypeScript parsed pages and cursor bytes agree, including retained
  forced updates, pruned old tips and history-unavailable error metadata.
- Unsigned ref changes produce no trigger until the signed set changes. This
  is reconciliation of observed frontiers, not a durable log of every write.

The independently written test oracle always enumerates `remotes()` and signed
sets, unions their COB targets, and diffs snapshots. The benchmark's
`signed-set-scan` mode is a simpler cost comparator: it disables the idle
shortcut and reloads every signed set through the same reducer. It is **not**
an independent implementation. Neither evaluates all typed COBs or walks
operation history. Tests compare against the independent oracle as well; the real-storage replay,
parent-cursor resume and copy-only retention pages were checked against it too.

Signed-set loading delegates signature/key/RID/replay checks to upstream; no
operation signatures or authorization are verified here. Each poll memoizes
sets by **namespace plus OID**, not OID alone. There is no cross-poll cache.
Signed names with `refs/cobs/<typename>/<id>` shape are retained, including
non-issue/patch COB types. Invalid advertised object IDs fail the poll as
`GitError` / `GIT`, not as caller argument errors. Object IDs are parsed;
upstream typename validation
has no public parser and is not reimplemented by the experiment.

## Real-storage measurements

Read-only probes of the host's local heartwood repository
`rad:z3gqcJUoA1n9HaHKufZs5FCSGazv5`: **3 namespaces, 2,495 signed ref names,
1,724 signed COB refs, 1,183 distinct COB names**. The programs were copied
outside the checkout and run through an optimized installed abi3 wheel on
CPython 3.14 and an optimized installed tarball on Node 26.5.0. Five warm
rounds, medians in milliseconds, same local Linux x86_64 GNU host; these are
not remote CI, other-platform, cold-cache or latency guarantees.

| Probe | Python tips / scan ms | Node tips / scan ms | Work per tips poll |
| --- | --- | --- | --- |
| Idle current cursor | 2.89 / 14.24 | 3.95 / 23.39 | 1 namespace-tip call; 0 sets, 0 commits, 0 signed ref names |
| Resume from each namespace's immediate parent | 14.68 / 14.73 | 26.43 / 19.94 | 6 sets, 4,988 names, 3 commit reads; 3 namespace records, 1 COB trigger |
| Replay from empty | 8.63 / 8.61 | 14.64 / 13.63 | 3 sets, 2,495 names; 3 namespace records, 1,183 COB triggers |

Current cursor: **423 bytes**, independent of COB history length. The delta
result deliberately shows no general changed-poll speedup: all old/new union
sets must be loaded whenever a namespace changes. Timing variance is visible
in the Node modes; do not interpret the warm sample as a ranking of languages.

Idle discovery no longer decodes signed ref names, but the backend ref glob
may still traverse the repository's ref database. One API call and three
returned tips are **not** proof of O(namespaces) filesystem work. Likewise,
legacy signed-set verification may traverse sigrefs history inside upstream.
The counters count SDK calls/decoded names, not physical I/O.

### Retention on disposable copies

`probe_reconciliation.py` copies the real repository into temporary profiles;
**all ref changes, pruning and object deletion happen only in copies**. It uses
no hardlinks/alternates and skips lock files. Source refs are checked before/
after; the source must be quiescent while copying. The optional Node probe
compares complete pages/counters, or checks the same Git error class.

| Copy-only case | Observed |
| --- | --- |
| Remove one namespace | `removed`; 784 COB triggers, only changed/deleted; shared COBs not falsely all deleted |
| Rewind its sigrefs tip to its parent, old tip still retained | namespace `forced`, **0 COB changes**; 2,281 commit reads to prove non-ancestry |
| Same, expire reflogs and `git gc --prune=now` | `history-unavailable`; 784 provisional created/changed triggers, all `old_complete: false` |
| Current sigrefs points at absent object | Git error, no page |
| Endpoints readable, intermediate current-chain commit genuinely omitted from repacked copy | Git error, no page |

The retained forced update took about **0.38 s in Python**, dominated by the
2,281 public `Repository.commit()` reads; Node end-to-end, including process
startup, was about **0.55 s**. Ordinary forward resume needed three reads.
A corrupt loose object was *not* a valid missing-object experiment: libgit2
could fall back to its intact packed copy. The committed probe instead rebuilds
the copy's object store omitting the selected object, and then both SDKs fail.

## Recommendation and remaining work

**The smaller feature makes sense without operation ranges.** Both primitives
are enough for a compact cursor and reliable reconciliation triggers, with
explicit gaps rather than invented complete history. It avoids operation
history work entirely, even on the 1,183-COB repository.

Keep operation ranges deferred. Before turning this application code into a
public feed, settle the wrapper/page schema and recovery ergonomics against a
real consumer. Namespace ancestry remains a potential bottleneck on forced or
long-downtime updates: a native plain-Git ancestry check could avoid thousands
of binding calls without introducing the deferred causal operation primitive.
Changed-poll signed-set caching is a separate optimization, not a requirement
or an honest O(changes) claim today. No production scalability claim follows
from these local probes.

## Reproduce

```sh
python examples/bench_reconciliation.py --rounds 5 rad:z...
node examples/bench-reconciliation.mjs --rounds 5 rad:z...
# For offline resume, supply an external old cursor to either benchmark:
python examples/bench_reconciliation.py --since /tmp/old.cursor rad:z...
# Retention writes ONLY disposable copies (needs local Git and enough disk):
python examples/probe_reconciliation.py --node-script examples/reconcile-refs.mjs rad:z...
```

Evidence for this run: `/tmp/radicle-reconciliation/heartwood-*.json`, including
`heartwood-retention-committed-final.json` and the `*-final.json` timing runs. Shared tests are
`tests/test_reconcile_refs.py`; Node's independent tests are
`node/test/reconcile-refs.test.mjs`. Existing observer/operation experiments
remain unchanged historical baselines. No SDK sources or locks are changed.
