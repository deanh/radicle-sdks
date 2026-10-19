// Native environment is fixed before this process and its worker realms start.
import assert from 'node:assert/strict';
import { Worker } from 'node:worker_threads';
const { path, peers } = JSON.parse(process.argv[2]);
const outcomes = await Promise.allSettled(
  peers.map(
    (peer) =>
      new Promise((resolve, reject) => {
        const worker = new Worker(new URL('./policy-worker.mjs', import.meta.url), {
          workerData: { path, ...peer },
        });
        const deadline = setTimeout(() => worker.terminate(), 30000);
        let message, failure;
        worker.once('error', (error) => {
          failure = error;
        });
        worker.once('message', (value) => {
          message = value;
        });
        worker.once('exit', (code) => {
          clearTimeout(deadline);
          if (code === 0 && !failure && message && !message.error) resolve(message);
          else reject(failure ?? new Error(JSON.stringify(message ?? { code })));
        });
      }),
  ),
);
const results = outcomes.map((outcome) => {
  assert.equal(outcome.status, 'fulfilled');
  return outcome.value;
});
process.stdout.write(JSON.stringify(results));
