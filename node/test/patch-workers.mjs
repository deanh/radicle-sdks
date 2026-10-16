// Fresh OS coordinator supplies RAD_HOME; each worker independently loads its
// own profile/repository/signer, exercising the process-wide write registry.
import assert from 'node:assert/strict';
import { once } from 'node:events';
import { isMainThread, parentPort, Worker, workerData } from 'node:worker_threads';
import { Profile, Patches } from '../dist/index.js';

if (isMainThread) {
  const workers = Array.from(
    { length: 3 },
    (_, i) =>
      new Worker(new URL(import.meta.url), {
        workerData: {
          rid: process.argv[2],
          id: process.argv[3],
          revision: process.argv[4],
          prefix: i,
        },
      }),
  );
  try {
    await Promise.all(workers.map((w) => once(w, 'message')));
    const pending = workers.map((w) => once(w, 'message'));
    workers.forEach((w) => w.postMessage('start'));
    const results = (await Promise.all(pending)).flatMap(([results]) => results);
    console.log(JSON.stringify(results));
  } finally {
    await Promise.all(workers.map((w) => w.terminate()));
  }
} else {
  const profile = Profile.load();
  const repo = profile.storage.repository(workerData.rid);
  const handle = await Patches.openAs(repo, profile.signer()).getMut(workerData.id);
  const start = once(parentPort, 'message');
  parentPort.postMessage('ready');
  await start;
  const results = await Promise.all(
    Array.from({ length: 4 }, (_, i) =>
      handle.comment(workerData.revision, `worker-${workerData.prefix}-${i}`),
    ),
  );
  for (const r of results) {
    assert.ok(Object.isFrozen(r));
    assert.ok(Object.isFrozen(r.snapshot.revisions));
    assert.ok(Object.isFrozen(r.snapshot.revisions[0].comments));
  }
  const signed = await repo.signRefs(profile.signer()); // independent signer mutex, same repository lock
  assert.ok(signed.refs[`refs/cobs/xyz.radicle.patch/${workerData.id}`]);
  assert.ok(Object.isFrozen(signed.refs));
  parentPort.postMessage(results);
  parentPort.close();
}
