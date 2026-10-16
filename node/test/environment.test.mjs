import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, rm, mkdir, writeFile, copyFile, stat } from 'node:fs/promises';
import { join } from 'node:path';
import { tmpdir } from 'node:os';
import { fileURLToPath } from 'node:url';
import { execFile, spawn } from 'node:child_process';
import { promisify } from 'node:util';
import { once } from 'node:events';
import { setTimeout as delay } from 'node:timers/promises';
import { Profile } from '../dist/index.js';
import { git } from './helpers.mjs';
const exec = promisify(execFile);

function environment(values) {
  const env = { ...process.env };
  for (const k of ['RAD_HOME', 'HOME', 'USERPROFILE', 'RAD_PASSPHRASE', 'SSH_AUTH_SOCK']) delete env[k];
  return { ...env, ...values };
}
async function consumer(env, data) {
  // Select native environment at OS process startup, never by global mutation.
  const { stdout } = await exec(process.execPath, [fileURLToPath(new URL('./environment-consumer.mjs', import.meta.url)), JSON.stringify(data)], {
    env, timeout: 20000,
  });
  assert.match(stdout, /Environment workflow OK/);
}
async function root(t) {
  const path = await mkdtemp(join(tmpdir(), 'sdk-env-'));
  t.after(() => rm(path, { recursive: true, force: true }));
  return path;
}

test('synchronous upstream profile/signer use native process environment and typed errors', { timeout: 30000 }, async t => {
  const path = await root(t), parent = join(path, 'a'), home = join(path, 'b');
  const encrypted = await Profile.init(join(parent, '.radicle'), 'alice', { passphrase: 't0-fixture-secret' });
  const plain = await Profile.init(home, 'bob');
  // HOME fallback, RAD_HOME priority, plaintext-key precedence, and a worker
  // whose conflicting JS env copy must not change upstream OS env selection.
  await Promise.all([
    consumer(environment({ HOME: parent, RAD_PASSPHRASE: 't0-fixture-secret' }), { nodeId: encrypted.nodeId }),
    consumer(environment({ RAD_HOME: home, HOME: '/ignored-home', RAD_PASSPHRASE: 'irrelevant', SSH_AUTH_SOCK: '/invalid-agent' }), { nodeId: plain.nodeId }),
    consumer(environment({ RAD_HOME: join(parent, '.radicle'), RAD_PASSPHRASE: 't0-fixture-secret' }), { nodeId: encrypted.nodeId, worker: true }),
  ]);
  await consumer(environment({ RAD_HOME: join(parent, '.radicle'), RAD_PASSPHRASE: 'wrong' }), { nodeId: encrypted.nodeId, badSigner: true });
  await consumer(environment({ RAD_HOME: join(parent, '.radicle'), RAD_PASSPHRASE: '' }), { nodeId: encrypted.nodeId, badSigner: true });
  await consumer(environment({}), { noHome: true });
  assert.equal((await stat(join(parent, '.radicle', 'keys', 'radicle'))).isFile(), true);
});

test('synchronous upstream encrypted signer uses the native agent and signs local writes', { timeout: 30000 }, async t => {
  if (process.platform === 'win32') return t.skip('Unix SSH-agent fixture');
  for (const tool of ['ssh-agent', 'ssh-add', 'ssh-keygen']) {
    try { await exec('which', [tool]); } catch { return t.skip(`${tool} unavailable`); }
  }
  const path = await root(t), home = join(path, 'home'), socket = join(path, 'agent.sock');
  const profile = await Profile.init(home, 'alice', { passphrase: 't0-fixture-secret' });
  // Register only a throwaway fixture key with a private agent, never the user's agent.
  const key = join(path, 'key');
  await copyFile(join(home, 'keys', 'radicle'), key);
  await exec('ssh-keygen', ['-p', '-P', 't0-fixture-secret', '-N', '', '-f', key]);
  const agent = spawn('ssh-agent', ['-D', '-a', socket], { stdio: 'ignore' });
  const closed = once(agent, 'close');
  t.after(async () => { agent.kill('SIGTERM'); await closed; });
  let ready = false;
  for (let i = 0; i < 200; i++) {
    try { ready = (await stat(socket)).isSocket(); } catch { /* not listening yet */ }
    if (ready || agent.exitCode !== null) break;
    await delay(10);
  }
  assert.ok(ready, 'isolated ssh-agent did not start');
  await exec('ssh-add', [key], { env: environment({ SSH_AUTH_SOCK: socket }) });
  const work = join(path, 'work');
  await mkdir(work);
  git(work, 'init', '--initial-branch=main');
  await writeFile(join(work, 'README.md'), '# Agent fixture\n');
  git(work, 'add', '.'); git(work, 'commit', '-m', 'initial');
  await consumer(environment({ RAD_HOME: home, RAD_PASSPHRASE: '', SSH_AUTH_SOCK: socket }), { nodeId: profile.nodeId, work, worker: true });
});
