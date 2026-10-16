import test from 'node:test';
import assert from 'node:assert/strict';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';

// A regression must fail this child, not abort the test runner itself.
test('worker and synchronous Rust panics cannot unwind through the Node boundary', async () => {
  const code = `
    import assert from 'node:assert/strict';
    import { probePanic, probeSyncPanic, probeTimestamp } from './node/dist/probes.js';
    import { RadicleError, Profile } from './node/dist/index.js';
    import { mkdtemp, rm } from 'node:fs/promises';
    import { tmpdir } from 'node:os';
    import { join } from 'node:path';
    const internal = e => e instanceof RadicleError && e.code === 'INTERNAL';
    await assert.rejects(probePanic(), e => internal(e) && !e.message.includes('injected worker'));
    assert.throws(() => probeSyncPanic(), internal);
    assert.equal(await probeTimestamp('42', false), 42);
    const home = await mkdtemp(join(tmpdir(), 'sdk-after-panic-'));
    try { assert.match((await Profile.init(home, 'alice')).did, /^did:key:/); }
    finally { await rm(home, {recursive:true, force:true}); }
    console.log('survived');
  `;
  const { stdout } = await promisify(execFile)(
    process.execPath,
    ['--input-type=module', '--eval', code],
    {
      cwd: new URL('../..', import.meta.url),
      timeout: 30000,
    },
  );
  assert.match(stdout, /survived/);
});
