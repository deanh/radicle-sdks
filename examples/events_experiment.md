# Events experiment: observations, operations and frontiers

Related issues: events design `089606916a4c782f0eb210cc9d8d5d3bb46ace33`,
Python spike `e911911af3c628fb93b14948145bf7158d7a057c`.
This is research application code, not an SDK event API or a production watcher.

Historical record: the amended direction in events issue `0896069`, comment
`af7b3f81d649a652b4be619fbe19d6136a23543a` (amending `bb370ff`), supersedes
this prototype's recovery and cursor policies. Here a problematic COB frontier
halts the whole poll with `ResyncRequired` and suppresses delivery; the intended
feed instead reports namespace-level history-unavailable conditions separately
from per-COB created/updated/forced/deleted records, and emits the `^old new`
operation range even for forced updates. It uses union-of-namespace tip sets and
a strict repository-bound refs-style text cursor, not this experiment's JSON
cursor. Signing metadata is read, not signature verification. Keep this code as
experimental evidence; implement the three read-only primitives in both bindings
before rebuilding the experiment and specifying an SDK event API.

## What is being compared

- `watch_issues.py` is the evaluated **appearance/state delivery baseline**:
  it reports newly observed issues/comments and remembers their IDs. It is not
  the operation feed chosen in 0896069 and does not satisfy that issue by itself.
- `operation_events.py --source scan` is the operation-level reference. It lists
  issue/patch refs using public Raw APIs, diffs aggregate tip frontiers, and reads
  changed COB histories. Unchanged refs require no history evaluation.
  `operation-events.mjs` is the TypeScript port of the same contract; cursors and
  parsed events are shared across languages (JSON key order differs by language).
- `operation_events.py --source sigrefs` is the compact-checkpoint candidate.
  Its cursor contains namespace -> sigrefs OID, not every ref tip/operation ID.
  It compares current namespace tips, then parses historical/current `refs` blobs
  to construct the old/new COB frontiers when a namespace changes.

```sh
python examples/operation_events.py --state /tmp/ops-scan.json --source scan rad:z...
python examples/operation_events.py --state /tmp/ops-signed.json --source sigrefs rad:z...
node examples/operation-events.mjs --state /tmp/ops-signed.json --source sigrefs rad:z...
# Initial replay instead of a silent baseline (only on a missing cursor):
python examples/operation_events.py --state /tmp/ops-replay.json --replay rad:z...
python examples/bench_events.py --rounds 5 rad:z...
```

Use an existing local profile via RAD_HOME. Checkpoints must be outside Radicle
home/storage with an existing parent. These programs load no signer and never
fetch, announce or write repository refs. The benchmark creates only temporary
external cursors. Keep the repository idle while measuring unchanged polls.

`poll_operations(profile, rid, filename, handle, source=..., replay=...)` is the
importable synchronous callback experiment. The CLI scans once, emits JSON-lines
on stdout, and reports call/count/time statistics on stderr. All reads and graph
checks precede callbacks; the cursor advances after the entire successful batch.
Delivery/checkpoint failures may repeat callbacks. The single-writer/path/atomic
replace limits of the [baseline](watch_issues.md) apply. Cursor model/source/scope
are validated; a state-observer file or another source mode is not silently reset.

## Vocabulary and validity findings

One event represents **one newly reachable operation**, keyed by its OID, not one
callback per action. It includes RID, COB type/entity ID, actor DID, author-claimed
raw seconds as a decimal string, causal parents, all payload actions, actionKinds,
and exact base64 rawActions. Root operations classify as issue.opened/patch.opened.
A single non-root action becomes e.g. issue.commentRecorded or patch.reviewRecorded;
a multi-action operation is issue.operation/patch.operation with indexed array
positions retained. Unknown action types are retained, not dropped. These names
are a vocabulary experiment, not an exhaustive frozen SDK enum.

Every event has `evaluation: "not-evaluated"`. An action being recorded does not
mean it was authorized/applied, or that the patch became merged. The test records
two reviews from one actor: the feed reports both operation IDs, while the typed
snapshot retains the first verdict/summary. Issue title edits made between polls
also remain separate operations. The existing appearance observer emits neither
(title edits/patch reviews are outside its restricted vocabulary). A broader
state-diff view could report final title/state, but cannot recover every
intermediate operation from final state alone.

Payload is a decoded JSON **preview**: unsafe Python integers/Node BigInts become
decimal strings for JSON output. Raw bytes remain authoritative for exact numeric
and lexical fidelity, especially outside Python serde_json's i64/u64 paths. An
invalid JSON action produces decodeError + rawActions rather than hiding the
operation. Raw loading/signature metadata and parsing a sigrefs blob are **not**
a new authentication API; this prototype does not verify arbitrary historical
sigrefs or operation signatures. Any eventual SDK contract must say what is
verified rather than treating a cursor or an actor field as proof.

Recommendation: choose operation-level granularity for the base feed, with all
actions and stable per-action indexes available. Put evaluated state-diff or an
explicit applied/evaluation layer above it. Do not silently filter ignored
operations or call a raw merge record "patch merged".

## Change detection and resume findings

Both sources produce identical events on the same **signed, fast-forward** issue/
patch fixtures, including multi-action creation and two-actor review histories,
in both languages and from either language's cursor. The cross-language test
resumes a Python cursor from Node and vice versa, compares full replay sets, and
replays a Python-produced delta through the Node CLI. Signing is a material
contract difference: a fixture advances a COB ref while restoring its previous
sigrefs. Scan sees it; sigrefs emits nothing until the new set is signed.
Re-signing the unchanged signed set still does not advertise it; the next signed
COB write does, delivering both operations. The candidate clips the returned
history to the selected signed tips, so it does not accidentally emit raw
descendants beyond that frontier.

The prototype cannot honestly implement all of the proposed efficient/lossless
resume contract using today's public exports:

1. `Repository.remotes()` discovers namespaces **and materializes their signed
   ref names** on every call. One counted call is not O(namespaces) backend work.
   Namespace discovery still scales with signed refs even on an idle poll.
2. `Raw.history()` returns the **whole currently reachable history**, not a walk
   bounded by previous tips. A one-comment delta after 11 operations loads 12,
   though only one event is emitted. Call counters show this amplification.
3. No public API loads an arbitrary old COB entry/frontier. After a signed rewind,
   removal, divergent replacement or GC of needed data, an old tip may not occur
   in current Raw.history. The experiment raises **ResyncRequired** and retains
   the cursor, before callbacks. It does not reset, claim lossless recovery, or
   silently replay uncertain history. Restoring the original frontier resumes
   without duplicate events. The appearance baseline separately proves that its
   remembered IDs suppress rediscovery after ref rewind/restoration.
4. An old sigrefs cursor requires its immutable blob to remain available; no
   retention/GC guarantee is added. Multiple calls do not form a transaction;
   concurrent changes may make the selected tips temporarily unavailable.

Recommendation: keep full scans as a reference oracle and compact sigrefs cursors
as the candidate, **without changing the chosen operation-feed direction**. Before
an SDK API, investigate minimal read-only primitives for lightweight namespace/
sigrefs-tip discovery, reading verified ref sets at a selected tip, and causal
history difference from explicit old/new frontiers. Decide retention, rewind/
removal recovery and signing scope explicitly. A reset/replay policy is an
application decision, not a hidden fallback. Shared core extraction is not a
prerequisite to that investigation.

## Measurements (local observations, not guarantees)

Python 3.14, Linux x86_64, installed optimized Rust-1.91 abi3 wheel; five warm idle
polls after a silent baseline. One machine/current SDK, no cold-cache or backend
I/O tracing. Fixtures were generated in temporary local storage with public SDK
writes; no network or real-repository mutation. Cursor sizes are compact JSON
bytes, not compressed size. Examples and SDK artifacts live on the same local
filesystem. `bench_events.measure` reproduces idle measurement on an existing RID.

| Issues | Comments/issue | Actors | State idle ms | Scan idle ms | Sigrefs idle ms | Cursor bytes state/scan/sigrefs |
| --- | --- | --- | --- | --- | --- | --- |
| 10 | 0 | 1 | 1.22 | 1.23 | 0.38 | 1006 / 1227 / 264 |
| 100 | 0 | 1 | 9.31 | 17.40 | 0.55 | 8926 / 10767 / 264 |
| 10 | 10 | 1 | 6.08 | 0.85 | 0.26 | 5307 / 1228 / 265 |
| 10 | 10 | 5 | 6.58 | 2.18 | 0.72 | 5307 / 2948 / 641 |
| Local radicle-sdks | existing data | 1 | 3.23 | 3.85 | 0.37 | 1970 / 3748 / 241 |

A one-comment delta after ten comments loaded 12 history entries in both operation
modes (one event): scan 1.42 ms, sigrefs 0.99 ms in the one-actor fixture; five actors
were 2.86/1.97 ms. Idle sigrefs discovery still materialized 14/104/14/58 ref names
for the four synthetic rows. These figures do **not** establish O(namespaces +
changes) or a general speedup. An unoptimized development/helper run was tens of
times slower in typed evaluation; optimization/build configuration must accompany
any performance claim. Evidence: `/tmp/radicle-operation-spike-python/` on the
development host (`measurements-release.json`, development comparison retained).

## Decision and remaining questions

These experiments support separating an operation feed from evaluated appearance
notifications and reusing the consumer delivery/checkpoint contract. They do not
justify promoting the current implementation into an SDK watcher. The next SDK
proposal should specify the missing read-only primitives and test signatures,
retention/rewind recovery, unknown/invalid actions and source scope. Per-repository
scope is sufficient here; storage-wide feeds and background node integration are
not implemented. No exactly-once, global chronological ordering, durable journal,
performance SLA or reliable detection of changes removed between polls is claimed.
