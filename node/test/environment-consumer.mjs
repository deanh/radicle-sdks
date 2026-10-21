import assert from 'node:assert/strict';
import { once } from 'node:events';
import { isMainThread, Worker, workerData } from 'node:worker_threads';
import { Profile, Signer, RadicleError, initProject, Issues } from '../dist/index.js';

const data = isMainThread ? JSON.parse(process.argv[2]) : workerData;
const category = (code) => (e) =>
  e instanceof RadicleError && e.code === code && e.cause instanceof Error;
if (data.worker) {
  // Worker-local JS env copies must not override upstream's native OS env.
  const worker = new Worker(new URL(import.meta.url), {
    env: {
      ...process.env,
      RAD_HOME: '/ignored-worker-home',
      HOME: '/ignored-worker-parent',
      RAD_PASSPHRASE: 'wrong-worker-secret',
      SSH_AUTH_SOCK: '/ignored-worker-socket',
    },
    workerData: { ...data, worker: false },
  });
  assert.equal((await once(worker, 'exit'))[0], 0);
} else if (data.noHome) {
  assert.throws(() => Profile.load(), category('IDENTITY'));
  await assert.rejects(Profile.loadAsync(), category('IDENTITY'));
} else {
  const profile = Profile.load();
  assert.ok(profile instanceof Profile); // not a Promise
  assert.equal(profile.nodeId, data.nodeId);
  const asyncProfile = await Profile.loadAsync();
  assert.ok(asyncProfile instanceof Profile);
  assert.equal(asyncProfile.nodeId, profile.nodeId);
  if (data.badSigner) {
    assert.throws(() => profile.signer(), category('SIGNER'));
    await assert.rejects(asyncProfile.signerAsync(), category('SIGNER'));
  } else {
    const syncSigner = profile.signer();
    assert.ok(syncSigner instanceof Signer); // not a Promise
    const signer = await asyncProfile.signerAsync();
    assert.ok(signer instanceof Signer);
    assert.equal(signer.nodeId, syncSigner.nodeId);
    assert.equal(signer.nodeId, data.nodeId);
    if (data.work) {
      const rid = await initProject(profile.storage, signer, data.work, {
        name: 'agent-fixture',
        description: 'upstream socket',
        defaultBranch: 'main',
      });
      const issue = await Issues.openAs(profile.storage.repository(rid), signer).create({
        title: 'Agent',
        description: 'Root',
      });
      const change = await issue.comment('Signed through upstream agent');
      assert.equal(change.snapshot.comments.length, 2);
    }
  }
}
console.log('Environment workflow OK');
