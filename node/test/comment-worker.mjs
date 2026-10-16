import { parentPort, workerData } from 'node:worker_threads';
import { Profile, Issues } from '../dist/index.js';

try {
  const profile = Profile.load();
  const repo = profile.storage.repository(workerData.rid);
  const signer = profile.signer();
  const handle = await Issues.openAs(repo, signer).getMut(workerData.id);
  parentPort.postMessage({ ready: true });
  parentPort.once('message', async () => {
    try {
      const pending = Array.from({ length: workerData.count }, (_, i) =>
        handle.comment(`${workerData.prefix}-${i}`),
      );
      parentPort.postMessage({ queued: true });
      const results = await Promise.all(pending);
      parentPort.postMessage({ results });
    } catch (e) {
      parentPort.postMessage({ error: String(e), stack: e.stack });
    }
    parentPort.close();
  });
} catch (e) {
  parentPort.postMessage({ error: String(e), stack: e.stack });
  parentPort.close();
}
