import test from 'node:test';
import assert from 'node:assert/strict';
import { join } from 'node:path';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { fileURLToPath } from 'node:url';
import { readFile } from 'node:fs/promises';
import { Issues, RadicleError, forkProject, checkoutProject, initProject, Profile } from '../dist/index.js';
import { fixture, git } from './helpers.mjs';

function category(code) {
  return e => e instanceof RadicleError && e.code === code && e.cause instanceof Error;
}
function checkChange(change) {
  assert.ok(Object.isFrozen(change));
  assert.ok(Object.isFrozen(change.snapshot));
  assert.ok(Object.isFrozen(change.snapshot.comments));
  assert.ok(change.snapshot.comments.some(c => c.id === change.entryId));
}

test('workflow 1: init, inspect, fork, checkout; detached copied bytes', async t => {
  const f = await fixture(t);
  assert.match(f.profile.did, /^did:key:/);
  assert.equal(f.profile.nodeId, f.signer.nodeId);
  const head = await f.repo.head();
  assert.ok(Object.isFrozen(head));
  assert.equal(head.oid, git(f.work, 'rev-parse', 'HEAD'));
  const identity = await f.repo.identity();
  assert.equal(identity.visibility, 'public');
  assert.equal(identity.name, 'fixture');
  assert.equal(identity.defaultBranch, 'main');
  assert.ok(Object.isFrozen(identity.delegates));
  const first = await f.repo.commit(head.oid), second = await f.repo.commit(head.oid);
  assert.ok(Object.isFrozen(first));
  assert.ok(first.message instanceof Uint8Array);
  assert.equal(typeof first.authorTime, 'number');
  assert.ok(Number.isSafeInteger(first.authorTime));
  assert.equal(new Date(first.authorTime).getTime(), first.authorTime);
  assert.equal(first.authorTime, Number(git(f.work, 'show', '-s', '--format=%at')) * 1000);
  assert.throws(() => { first.message = new Uint8Array(); }, TypeError);
  first.message.fill(0); first.authorName.fill(0);
  assert.equal(new TextDecoder().decode(second.message), 'initial\n');
  assert.notDeepEqual(first.message, second.message);
  assert.deepEqual((await f.repo.commit(head.oid)).message, second.message);
  assert.equal(git(f.work, 'remote', 'get-url', 'rad'), `rad://${f.rid.slice(4)}`);
  const originalRemote = git(f.work, 'config', '--get-regexp', '^remote.rad.');
  await assert.rejects(initProject(f.storage, f.signer, f.work, { name: 'again', description: 'no', defaultBranch: 'main' }), category('REPOSITORY'));
  assert.equal(git(f.work, 'config', '--get-regexp', '^remote.rad.'), originalRemote);
  await forkProject(f.storage, f.signer, f.rid);
  const checkout = join(f.root, 'checkout');
  assert.equal(await checkoutProject(f.storage, f.rid, f.signer.nodeId, checkout), checkout);
  assert.equal(git(checkout, 'rev-parse', 'HEAD'), head.oid);
  assert.equal(await readFile(join(checkout, 'README.md'), 'utf8'), '# Fixture\n');
  // No API constructed or contacted a Radicle node.
});

test('issue create and concurrent comments across same/separate handles; failure recovery', async t => {
  const f = await fixture(t);
  const issues = Issues.openAs(f.repo, f.signer);
  const issue = await issues.create({ title: 'First', description: 'Root' });
  assert.equal('issue' in issue, false);
  const before = await issue.reload();
  assert.equal(before.comments.length, 1);
  assert.equal(before.state, 'open');
  const other = await Issues.openAs(f.storage.repository(f.rid), f.profile.signer()).getMut(issue.id);
  const results = await Promise.allSettled([
    issue.comment('a'), issue.comment('b'),
    other.comment('bad reply', { replyTo: 'f'.repeat(40) }), other.comment('c'),
  ]);
  assert.equal(results[2].status, 'rejected');
  assert.equal(results[2].reason.code, 'COB');
  for (const r of results.filter(r => r.status === 'fulfilled')) checkChange(r.value);
  const last = await other.comment('after failure');
  checkChange(last);
  const now = await issue.reload();
  assert.deepEqual(new Set(now.comments.map(c => c.body)), new Set(['Root', 'a', 'b', 'c', 'after failure']));
  assert.equal(before.comments.length, 1);
  assert.equal(last.snapshot.comments.length, 5);
  assert.deepEqual(await Issues.open(f.repo).get(issue.id), now);
  assert.throws(() => now.comments.push({}), TypeError);
  assert.throws(() => { now.comments[0].body = 'changed'; }, TypeError);
  assert.equal(await Issues.open(f.repo).get('0'.repeat(40)), null);
});

test('process-wide locking across worker_threads and independently loaded profiles', { timeout: 60000 }, async t => {
  const f = await fixture(t);
  const issue = await Issues.openAs(f.repo, f.signer).create({ title: 'Workers', description: 'root' });
  const { stdout } = await promisify(execFile)(process.execPath, [
    fileURLToPath(new URL('./worker-coordinator.mjs', import.meta.url)), f.rid, issue.id,
  ], { env: { ...process.env, RAD_HOME: f.home }, timeout: 50000, maxBuffer: 1024*1024 });
  const { results, elapsed, read } = JSON.parse(stdout);
  t.diagnostic(`12 cross-worker writes: ${elapsed.toFixed(1)}ms; concurrent read completion: ${read.toFixed(1)}ms; UV_THREADPOOL_SIZE=${process.env.UV_THREADPOOL_SIZE || 'default (4)'}`);
  assert.equal(results.length, 12);
  const lengths = results.map(r => r.snapshot.comments.length).sort((a,b) => a-b);
  assert.deepEqual(lengths, Array.from({ length: 12 }, (_, i) => i+2));
  for (const result of results) assert.ok(result.snapshot.comments.some(c => c.id === result.entryId));
  const after = await issue.reload();
  assert.equal(after.comments.length, 13);
  assert.equal(new Set(after.comments.map(c => c.body)).size, 13);
});

test('real typed errors from async tasks and synchronous argument validation', async t => {
  const f = await fixture(t);
  await assert.rejects(Issues.open(f.repo).create({ title: 'no', description: '' }), category('READ_ONLY'));
  await assert.rejects(Issues.openAs(f.repo, f.signer).create({ title: '', description: '' }), e => e instanceof TypeError && e.code === 'ERR_INVALID_ARG_VALUE');
  await assert.rejects(f.repo.commit('garbage'), e => e instanceof TypeError && e.code === 'ERR_INVALID_ARG_VALUE');
  const invalidType = f.repo.commit(42);
  assert.ok(invalidType instanceof Promise);
  await assert.rejects(invalidType, e => e instanceof TypeError && e.code === 'ERR_INVALID_ARG_TYPE');
  await assert.rejects(f.repo.commit('0'.repeat(40)), category('GIT'));
  assert.throws(() => f.storage.repository('garbage'), e => e instanceof TypeError);
  await assert.rejects(Profile.init(join(f.root, 'invalid'), ''), e => e instanceof TypeError);
});
