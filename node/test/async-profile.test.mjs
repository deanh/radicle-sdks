import test from 'node:test';
import assert from 'node:assert/strict';
import { cp } from 'node:fs/promises';
import { join } from 'node:path';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { fileURLToPath } from 'node:url';
import { Profile, Issues } from 'radicle-sdk';
import { fixture } from './helpers.mjs';

test('async profile/signer setup queues real native work, decrypts and signs; errors reject', async (t) => {
  const f = await fixture(t);
  const home = join(f.root, 'encrypted');
  const encrypted = await Profile.init(home, 'encrypted', { passphrase: 'fixture-only' });
  await cp(join(f.storage.path, f.rid.slice(4)), join(encrypted.storage.path, f.rid.slice(4)), {
    recursive: true,
  });
  const env = {
    ...process.env,
    RAD_HOME: home,
    RAD_PASSPHRASE: 'fixture-only',
    UV_THREADPOOL_SIZE: '1',
  };
  delete env.SSH_AUTH_SOCK;
  const worker = fileURLToPath(new URL('./async-profile-worker.mjs', import.meta.url));
  const run = (mode, overrides = {}) =>
    promisify(execFile)(process.execPath, [worker, mode, f.rid], {
      env: { ...env, ...overrides },
      timeout: 60000,
    });
  const result = JSON.parse((await run(f.root)).stdout);
  assert.equal(result.did, encrypted.did);
  assert.equal(result.nodeId, encrypted.nodeId);
  assert.equal(result.queued, true);
  assert.equal(
    (await Issues.open(encrypted.storage.repository(f.rid)).get(result.issueId)).author,
    encrypted.did,
  );
  await run('bad-passphrase', { RAD_PASSPHRASE: 'wrong' });
  await run('missing', { RAD_HOME: join(f.root, 'missing') });
});
