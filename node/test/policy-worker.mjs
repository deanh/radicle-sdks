import { parentPort, workerData } from 'node:worker_threads';
import { PolicyStore, Profile } from 'radicle-sdk';
try {
  const policy = await PolicyStore.open(workerData.path);
  await policy.follow(workerData.nodeId, { alias: workerData.alias });
  await policy.setFollowPolicy(workerData.nodeId, 'block');
  await policy.follow(workerData.nodeId, { alias: workerData.alias });
  const routing = await Profile.load().routingStore();
  parentPort.postMessage({
    policy: await policy.followPolicy(workerData.nodeId),
    total: await routing.total(),
  });
} catch (error) {
  parentPort.postMessage({ error: { name: error.name, code: error.code, message: error.message } });
}
