import test from 'node:test';
import assert from 'node:assert/strict';
import { join, relative } from 'node:path';
import { access, writeFile, unlink, symlink } from 'node:fs/promises';
import { spawn, execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { fileURLToPath } from 'node:url';
import { DatabaseSync } from 'node:sqlite';
import { Profile, PolicyStore, RadicleError } from 'radicle-sdk';
import { fixture } from './helpers.mjs';

const category = (code) => (e) => e instanceof RadicleError && e.code === code;
const invalid = (e) => e instanceof TypeError && e.code === 'ERR_INVALID_ARG_VALUE';
const wrongType = (e) => e instanceof TypeError && e.code === 'ERR_INVALID_ARG_TYPE';

test('policy raw tables preserve follow/seed/block booleans, fuzzy aliases and frozen values', async (t) => {
  const f = await fixture(t),
    policy = await f.profile.policyStore();
  const reader = await PolicyStore.reader(f.profile);
  assert.equal(policy.path, join(f.home, 'node', 'policies.db'));
  assert.deepEqual(await policy.followPolicies(), []);
  assert.deepEqual(await policy.seedPolicies(), []);
  assert.equal(await policy.followPolicy(f.profile.nodeId), null);
  assert.equal(await policy.seedPolicy(f.rid), null);
  assert.equal(await policy.isBlocked(f.profile.nodeId), false);
  assert.equal(await policy.follow(f.profile.nodeId, { alias: 'Alice' }), true);
  assert.equal(await policy.follow(f.profile.nodeId, { alias: 'Alice' }), false);
  const info = await reader.followPolicy(f.profile.nodeId);
  assert.deepEqual(info, { nodeId: f.profile.nodeId, alias: 'Alice', policy: 'allow' });
  assert.ok(Object.isFrozen(info));
  assert.equal(await reader.isFollowing(f.profile.nodeId), true);
  const matches = await policy.nodesByAlias('LIC');
  assert.deepEqual(matches, [[f.profile.nodeId, 'Alice']]);
  assert.ok(Object.isFrozen(matches));
  assert.ok(Object.isFrozen(matches[0]));
  assert.deepEqual(await policy.nodesByAlias('nobody'), []);
  assert.equal(await policy.unblockNid(f.profile.nodeId), false);
  assert.equal(await policy.setFollowPolicy(f.profile.nodeId, 'block'), true);
  assert.equal(await policy.setFollowPolicy(f.profile.nodeId, 'block'), false);
  assert.equal(await policy.isBlocked(f.profile.nodeId), true);
  assert.equal(await policy.isFollowing(f.profile.nodeId), false);
  assert.equal(await policy.unfollow(f.profile.nodeId), true); // deletes block too
  assert.equal(await policy.unfollow(f.profile.nodeId), false);
  assert.equal(await policy.followPolicy(f.profile.nodeId), null);
  await policy.setFollowPolicy(f.profile.nodeId, 'block');
  assert.equal(await policy.unblockNid(f.profile.nodeId), true);
  assert.equal(await policy.unblockNid(f.profile.nodeId), false);
  assert.equal(await policy.seed(f.rid), true);
  assert.equal(await policy.seed(f.rid), false);
  assert.deepEqual(await reader.seedPolicy(f.rid), { rid: f.rid, policy: 'allow', scope: 'all' });
  assert.equal(await policy.seed(f.rid, { scope: 'followed' }), true);
  assert.equal(await policy.unblockRid(f.rid), false);
  assert.equal(await policy.setSeedPolicy(f.rid, 'block'), true);
  const blocked = await policy.seedPolicy(f.rid);
  assert.deepEqual(blocked, { rid: f.rid, policy: 'block' });
  assert.ok(Object.isFrozen(blocked));
  assert.equal(await policy.isSeeding(f.rid), false);
  // Upstream seed changes scope, not a block policy; do not synthesize allow.
  assert.equal(await policy.seed(f.rid, { scope: 'followed' }), false);
  assert.equal(await policy.isSeeding(f.rid), false);
  assert.equal(await policy.unseed(f.rid), true); // deletes block too
  assert.equal(await policy.unseed(f.rid), false);
  await policy.setSeedPolicy(f.rid, 'block');
  assert.equal(await policy.unblockRid(f.rid), true);
  assert.equal(await policy.unblockRid(f.rid), false);
  await policy.setSeedPolicy(f.rid, 'allow');
  assert.equal(await policy.isSeeding(f.rid), true);
  assert.ok(Object.isFrozen(await policy.seedPolicies()));
});

test('policy factories validate eagerly; read-only open never creates or repairs a database', async (t) => {
  const f = await fixture(t);
  const reader = await PolicyStore.reader(f.profile);
  for (const [method, args] of [
    ['follow', [f.profile.nodeId]],
    ['unfollow', [f.profile.nodeId]],
    ['seed', [f.rid]],
    ['unseed', [f.rid]],
    ['setFollowPolicy', [f.profile.nodeId, 'allow']],
    ['setSeedPolicy', [f.rid, 'allow']],
    ['unblockRid', [f.rid]],
    ['unblockNid', [f.profile.nodeId]],
  ]) {
    await assert.rejects(reader[method](...args), category('READ_ONLY'));
  }
  await assert.rejects(reader.follow('invalid'), invalid); // validation before access check
  const path = join(f.home, 'node', 'policies.db');
  await unlink(path);
  await assert.rejects(PolicyStore.reader(f.profile), category('POLICY'));
  await assert.rejects(reader.followPolicies(), category('POLICY'));
  await assert.rejects(reader.follow(f.profile.nodeId), category('READ_ONLY'));
  await assert.rejects(access(path));
  const writable = await PolicyStore.fromProfile(f.profile);
  assert.deepEqual(await reader.followPolicies(), []); // reopened, sees replacement file
  await unlink(path);
  await assert.rejects(writable.followPolicies(), category('POLICY')); // no create on reads either
  await assert.rejects(access(path));
  assert.equal(await writable.follow(f.profile.nodeId), true); // authorized write can recreate
  const empty = join(f.root, 'empty.db');
  await unlink(path);
  await writeFile(path, '');
  await assert.rejects(PolicyStore.reader(f.profile), category('POLICY')); // missing schema, no migration
  const db = new DatabaseSync(path);
  assert.deepEqual(db.prepare('SELECT name FROM sqlite_master').all(), []);
  db.close();
  assert.deepEqual(await (await PolicyStore.open(empty)).followPolicies(), []);
  await assert.rejects(
    PolicyStore.open(join(f.root, 'missing-parent', 'policies.db')),
    category('POLICY'),
  );
  await assert.rejects(PolicyStore.open(42), wrongType);
  await assert.rejects(PolicyStore.open(''), invalid);
  await assert.rejects(PolicyStore.open(':memory:'), invalid);
  await assert.rejects(PolicyStore.fromProfile({}), wrongType);
  await assert.rejects(PolicyStore.reader(null), wrongType);
});

test('policy reopening pins relative/symlink paths; validators and corrupt rows do not hide failures', async (t) => {
  const f = await fixture(t),
    path = join(f.root, 'explicit.db');
  const policy = await PolicyStore.open(relative(process.cwd(), path));
  assert.equal(policy.path, path);
  const link = join(f.root, 'link.db');
  await symlink(path, link);
  assert.equal((await PolicyStore.open(link)).path, path);
  for (const name of [
    'follow',
    'unfollow',
    'isFollowing',
    'isBlocked',
    'followPolicy',
    'unblockNid',
  ]) {
    await assert.rejects(policy[name]('bad-id'), invalid);
    await assert.rejects(policy[name](null), wrongType);
  }
  for (const name of ['seed', 'unseed', 'isSeeding', 'seedPolicy', 'unblockRid']) {
    await assert.rejects(policy[name]('bad-rid'), invalid);
    await assert.rejects(policy[name](42), wrongType);
  }
  for (const opts of [null, [], { alias: null }, { alias: 1 }])
    await assert.rejects(policy.follow(f.profile.nodeId, opts), wrongType);
  for (const opts of [null, [], { scope: null }, { scope: false }])
    await assert.rejects(policy.seed(f.rid, opts), wrongType);
  await assert.rejects(policy.follow(f.profile.nodeId, { alias: 'bad alias!' }), invalid);
  await assert.rejects(policy.seed(f.rid, { scope: 'sometimes' }), invalid);
  await assert.rejects(policy.setFollowPolicy(f.profile.nodeId, 'permit'), invalid);
  await assert.rejects(policy.setSeedPolicy(f.rid, 'permit'), invalid);
  await assert.rejects(policy.setFollowPolicy(f.profile.nodeId, null), wrongType);
  await assert.rejects(policy.nodesByAlias('bad alias!'), invalid);
  assert.deepEqual(await policy.followPolicies(), []);
  const db = new DatabaseSync(path);
  try {
    db.prepare('INSERT INTO following(id, policy) VALUES (?, ?)').run(f.profile.nodeId, 'bogus');
    await assert.rejects(policy.followPolicy(f.profile.nodeId), category('POLICY'));
    await assert.rejects(policy.followPolicies(), category('POLICY'));
    db.exec('DELETE FROM following');
    assert.equal(await policy.follow(f.profile.nodeId), true);
  } finally {
    db.close();
  }
});

test('SQLite busy failures propagate without application retries and later operations recover', async (t) => {
  const f = await fixture(t),
    policy = await f.profile.policyStore();
  // Expected duration: about 6 seconds, waiting out upstream's write timeout.
  // Hold the lock in a different process. Independent SQLite builds inside one
  // process have separate VFS lock registries, so node:sqlite is not a valid
  // in-process lock arbiter for the addon's bundled SQLite.
  const child = spawn(
    process.execPath,
    [
      '--input-type=module',
      '-e',
      `
import {DatabaseSync} from 'node:sqlite';
const db = new DatabaseSync(process.argv[1]); db.exec('BEGIN EXCLUSIVE');
process.send('locked');
process.on('message', () => { db.exec('ROLLBACK'); db.close(); process.exit(0); });
`,
      policy.path,
    ],
    { stdio: ['ignore', 'ignore', 'pipe', 'ipc'] },
  );
  const exited = new Promise((resolve, reject) => {
    child.once('error', reject);
    child.once('exit', (code) =>
      code === 0 ? resolve() : reject(new Error(`lock holder exit ${code}`)),
    );
  });
  const deadline = setTimeout(() => child.kill(), 30000);
  deadline.unref();
  try {
    await Promise.race([
      new Promise((resolve, reject) => {
        child.once('message', resolve);
        child.once('error', reject);
      }),
      exited.then(() => {
        throw new Error('lock holder exited before ready');
      }),
    ]);
    await assert.rejects(policy.follow(f.profile.nodeId), category('POLICY'));
  } finally {
    clearTimeout(deadline);
    if (child.connected) child.send('release');
    await exited;
  }
  assert.equal(await policy.follow(f.profile.nodeId), true);
});

test('independent handles and worker realms use per-operation policy connections', async (t) => {
  const f = await fixture(t),
    policy = await f.profile.policyStore();
  const stores = await Promise.all(Array.from({ length: 8 }, () => PolicyStore.open(policy.path)));
  const changes = await Promise.all(stores.map((s) => s.follow(f.profile.nodeId)));
  assert.equal(changes.filter(Boolean).length, 1);
  const peers = await Promise.all(
    Array.from({ length: 3 }, (_, i) => Profile.init(join(f.root, `peer-${i}`), `peer-${i}`)),
  );
  const { stdout } = await promisify(execFile)(
    process.execPath,
    [
      fileURLToPath(new URL('./policy-coordinator.mjs', import.meta.url)),
      JSON.stringify({
        path: policy.path,
        peers: peers.map((p, i) => ({ nodeId: p.nodeId, alias: `worker-${i}` })),
      }),
    ],
    { env: { ...process.env, RAD_HOME: f.home }, timeout: 45000 },
  );
  const results = JSON.parse(stdout);
  for (let i = 0; i < results.length; i++) {
    assert.deepEqual(results[i], {
      policy: { nodeId: peers[i].nodeId, alias: `worker-${i}`, policy: 'allow' },
      total: 0,
    });
  }
  assert.equal((await policy.followPolicies()).length, 4);
});
