// Native environment and UV_THREADPOOL_SIZE are configured before process start.
import assert from 'node:assert/strict';
import { open, close, openSync, closeSync } from 'node:fs';
import { promisify } from 'node:util';
import { execFileSync } from 'node:child_process';
import { join } from 'node:path';
import { Profile, Issues } from 'radicle-sdk';

if (process.argv[2] === 'missing') {
  const promise = Profile.loadAsync();
  assert.ok(promise instanceof Promise);
  await assert.rejects(promise, (e) => e.code === 'IDENTITY');
} else if (process.argv[2] === 'bad-passphrase') {
  const profile = await Profile.loadAsync();
  await assert.rejects(profile.signerAsync(), (e) => e.code === 'SIGNER');
} else {
  // Occupy the sole libuv worker with an actual blocking FIFO open. JS releases
  // it explicitly: no sleep/decryption-duration assumptions or test SDK hooks.
  let counter = 0;
  async function queued(action) {
    const fifo = join(process.argv[2], `gate-${counter++}`);
    execFileSync('mkfifo', [fifo]);
    const reader = promisify(open)(fifo, 'r');
    let settled = false;
    const result = action().finally(() => {
      settled = true;
    });
    try {
      await new Promise(setImmediate);
      assert.equal(
        settled,
        false,
        'setup must be a queued native job, not synchronous I/O in a Promise',
      );
    } finally {
      const writer = openSync(fifo, 'w');
      closeSync(writer);
      await promisify(close)(await reader);
    }
    return result;
  }
  const profile = await queued(() => Profile.loadAsync());
  assert.ok(profile instanceof Profile);
  const signer = await queued(() => profile.signerAsync());
  assert.equal(signer.nodeId, profile.nodeId);
  assert.equal(signer.did, profile.did);
  assert.equal(Profile.load().did, profile.did); // original synchronous API remains
  const rid = process.argv[3];
  const issue = await Issues.openAs(profile.storage.repository(rid), signer).create({
    title: 'async setup',
    description: 'real signed fixture',
  });
  assert.equal((await issue.reload()).author, profile.did);
  console.log(
    JSON.stringify({ did: profile.did, nodeId: signer.nodeId, issueId: issue.id, queued: true }),
  );
}
