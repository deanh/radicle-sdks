# Local issue/comment observer spike

Tracking issues: Python `e911911af3c628fb93b14948145bf7158d7a057c` and
TypeScript `856fc04543eb5c59e70b8294fbd40f74c641c687`.
[`watch_issues.py`](watch_issues.py) and [`watch-issues.mjs`](watch-issues.mjs) use
installed **public** `radiclepy` / `radicle-sdk` exports. They share the event and
version-1 checkpoint format; either can resume the other's checkpoint (never run
both writers on the same file). These are application spikes, not SDK event APIs.

## Run

Set `RAD_HOME` before starting the process, as with the SDK guides. Choose one
repository already in its storage and a checkpoint path whose parent exists,
**outside** Radicle home/storage. The program never loads a signer or writes
repository data. It writes only its caller-owned checkpoint.

```sh
python examples/watch_issues.py --state /tmp/my-issues.json --once rad:z...
python examples/watch_issues.py --state /tmp/my-issues.json --interval 2 rad:z...
# Opt in to reporting existing data when the checkpoint does not yet exist:
python examples/watch_issues.py --state /tmp/my-replay.json --once --replay rad:z...
```

The default first scan silently establishes a baseline. Later scans print one
JSON object per line for `issue.discovered` and `comment.observed`. A newly
observed issue emits discovery, followed by its visible non-root comments. The
root description is not reported again as a comment. Closed issues are included.
A repository may contain old data just fetched from elsewhere: **observed does
not mean created now**. No node connection, fetch, announcement or daemon is used.

To execute application code, import `poll_once` and pass a synchronous callable:

```python
# With examples/ on your import path, and an existing profile/RID/checkpoint:
from watch_issues import poll_once

def handle(event):
    print(event["type"], event["id"])

poll_once(profile, rid, checkpoint, handle)
```

Importing the module does not load a profile or start a loop. Async callbacks
are rejected rather than silently checkpointed. Handlers receive ordinary event
dictionaries after all SDK reads have completed, without repository locks held.
The CLI stops on read, handler, output or checkpoint errors; it does not retry
application side effects. Ctrl-C exits; restart using the same checkpoint.

## TypeScript counterpart

```sh
node examples/watch-issues.mjs --state /tmp/my-issues.json --once rad:z...
node examples/watch-issues.mjs --state /tmp/my-issues.json --interval 2 rad:z...
node examples/watch-issues.mjs --state /tmp/my-replay.json --once --replay rad:z...
```

Importing `pollOnce` does not load a profile or start a loop:

```js
import { pollOnce } from './examples/watch-issues.mjs';
await pollOnce(profile, rid, checkpoint, async event => {
  console.log(event.type, event.id);
});
```

Both synchronous and asynchronous handlers are supported; each is awaited before
the next one. SIGINT/SIGTERM request a graceful stop after the current batch;
the interval sleep is cancellable. Python's Ctrl-C can instead interrupt the
current batch, leaving its checkpoint unchanged. CLI argument-error exit codes
and JSON whitespace/key ordering are language-specific; parsed event data agree.
Both CLIs fail rather than silently acknowledge handler/output/checkpoint errors.

## Event/checkpoint contract

Events share `id`, `type`, `rid`, `issueId`, `title`, `author`. Comment events add
`commentId` and `body`. Discovery IDs are `<RID>/issue/<issue-ID>`; comment IDs
append `/comment/<comment-ID>`. IDs are stable deduplication keys, **not a global
sequence or immutable historical payload**: titles/bodies can change on retry.
Events use evaluated typed issue/comment snapshots, not unauthenticated raw
comment actions. The checkpoint records observed entity/comment IDs, not every
raw COB operation. It is not a complete change log.

Version-1 checkpoint shape is `{version, storage, rid, seen}`, where `storage` is
the canonical storage path and `seen` maps issue IDs to sorted comment-ID arrays.
Wrong scopes, malformed files/IDs and unsupported versions fail closed; they are
never silently reset. Missing checkpoints baseline unless `--replay` is set;
that flag does not replay an existing checkpoint. If an initial replay fails
before creating the checkpoint, restart with `--replay` again; omitting it
intentionally establishes a new silent baseline. Keep a separate file per RID.
Existing symlink checkpoints and paths inside Radicle data are rejected. Parent
symlinks are resolved before the location check. This is accidental-overwrite
protection, **not a hostile-filesystem sandbox**.

Only after **every callback in the batch succeeds** is the checkpoint replaced
using a same-directory temporary file, file fsync and rename. No change means no
rewrite. Failure/crash before replacement leaves the previous baseline and may
repeat already delivered events on restart. Handlers must deduplicate/idempotently
process `id`; there is **no exactly-once guarantee**, transactional acknowledgement,
directory-fsync durability promise or automatic rollback. Use **one writer per
checkpoint**; no multi-process file lock is implemented.

## Deliberate spike limits

- Polling, one local repository, eager issue listings, memory/checkpoint growth
  proportional to all observed IDs. No scalability or latency guarantee.
- Issue IDs are sorted; comments retain upstream thread order. This is not a
  repository-wide chronological order or atomic snapshot across processes.
- Edits, redactions, deletions, patch activity and canonical-ref updates are not
  events here. A comment added and removed between scans can be missed. Removed
  IDs remain remembered so restoring refs does not automatically replay them.
- Callback code can use the SDK, but this example never replies. A handler that
  writes comments needs its own author filtering, feedback-loop prevention,
  budgets and side-effect deduplication; do not blindly reply to every event.
- Never execute repository text as code/shell commands. Default output is JSON
  data, including potentially untrusted peer text, not executable instructions.

Tests execute both actual CLIs and importable callback APIs against public SDK
fixtures, compare same-storage event data and cross-language checkpoint handoff, including baseline/replay, restart, failure, invalid state and unchanged
Git refs. No production watcher/reliable message-delivery contract is claimed.
