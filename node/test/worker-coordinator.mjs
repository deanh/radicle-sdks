// A fresh OS process gives Rust std::env the fixture RAD_HOME. Node per-worker
// JS env overrides are not reflected in the native process environment.
import { Worker } from 'node:worker_threads';
import { once } from 'node:events';
import { performance } from 'node:perf_hooks';
import { Profile, Issues } from '../dist/index.js';

const profile = Profile.load();
const repo = profile.storage.repository(process.argv[2]);
const issue = await Issues.openAs(repo, profile.signer()).getMut(process.argv[3]);
const workers = Array.from({ length: 3 }, (_, i) => new Worker(new URL('./comment-worker.mjs', import.meta.url), {
  workerData: { rid: repo.rid, id: issue.id, prefix: `worker-${i}`, count: 4 },
}));
try {
  await Promise.all(workers.map(async w => {
    const [m] = await once(w, 'message');
    if (!m.ready) throw new Error(m.stack || JSON.stringify(m));
  }));
  function waitFor(w, field) {
    return new Promise((resolve, reject) => {
      const done = () => { w.off('message', message); w.off('error', fail); };
      const fail = error => { done(); reject(error); };
      const message = m => {
        if (m.error) fail(new Error(m.stack || m.error));
        else if (field in m) { done(); resolve(m[field]); }
      };
      w.on('message', message); w.once('error', fail);
    });
  }
  const queued = workers.map(w => waitFor(w, 'queued'));
  const outputs = workers.map(w => waitFor(w, 'results'));
  const start = performance.now();
  workers.forEach(w => w.postMessage('go'));
  // Queue the read only after every worker submitted its burst; otherwise the
  // read can win the scheduling race and never observe pool contention.
  await Promise.all(queued);
  const readStart = performance.now();
  const read = issue.reload().then(() => performance.now() - readStart);
  const results = (await Promise.all(outputs)).flat();
  console.log(JSON.stringify({ results, elapsed: performance.now()-start, read: await read }));
} finally { await Promise.all(workers.map(w => w.terminate())); }
