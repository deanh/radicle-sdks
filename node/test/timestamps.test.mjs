import test from 'node:test';
import assert from 'node:assert/strict';
import { probeTimestamp } from '../dist/probes.js';
import { RadicleError, Issues, Patches } from '../dist/index.js';
import { execFile, execFileSync } from 'node:child_process';
import { promisify } from 'node:util';
import { fileURLToPath } from 'node:url';
import { join } from 'node:path';
import { fixture, git } from './helpers.mjs';

test('worker timestamp conversion: Date boundaries, multiplication overflow, u64 maximum', async () => {
  for (const [value, seconds, want] of [
    ['0', false, 0],
    ['1700000000', true, 1700000000000],
    ['-1', true, -1000],
    ['8640000000000000', false, 8640000000000000],
    ['-8640000000000000', false, -8640000000000000],
    ['8640000000000', true, 8640000000000000],
    ['-8640000000000', true, -8640000000000000],
  ]) {
    const result = await probeTimestamp(value, seconds);
    assert.equal(result, want);
    assert.equal(typeof result, 'number');
    assert.equal(new Date(result).getTime(), result);
    assert.ok(Number.isSafeInteger(result));
  }
  for (const [value, seconds] of [
    ['8640000000000001', false],
    ['-8640000000000001', false],
    ['8640000000001', true],
    ['-8640000000001', true],
    ['9223372036854775807', true],
    ['-9223372036854775808', true],
  ]) {
    await assert.rejects(
      probeTimestamp(value, seconds),
      (e) => e instanceof RadicleError && e.code === 'INVALID_DATA',
    );
  }
  // The probe accepts decimal i64 upstream values; overflow at its input boundary
  // exercises RangeError translation, not a public timestamp-input SDK feature.
  await assert.rejects(
    probeTimestamp('18446744073709551615', false),
    (e) => e instanceof RangeError && e.code === 'ERR_OUT_OF_RANGE',
  );
  await assert.rejects(
    probeTimestamp('bad', true),
    (e) => e instanceof TypeError && e.code === 'ERR_INVALID_ARG_VALUE',
  );
});

test('signed peer dates outside Date range remain readable and cannot deny issue listing', async (t) => {
  const f = await fixture(t),
    read = Issues.open(f.repo);
  const normal = await Issues.openAs(f.repo, f.signer).create({
    title: 'Normal',
    description: 'Normal root',
  });
  const before = await normal.reload();
  const ids = [];
  // Just outside Date's range, and a valid upstream millis value above i64::MAX.
  for (const seconds of ['8640000000001', '18446744073709551']) {
    const { stdout } = await promisify(execFile)(
      process.execPath,
      [fileURLToPath(new URL('./metadata-consumer.mjs', import.meta.url)), f.rid],
      { env: { ...process.env, RAD_HOME: f.home, GIT_COMMITTER_DATE: seconds }, timeout: 15000 },
    );
    const id = stdout.trim();
    assert.match(id, /^[0-9a-f]{40}$/);
    ids.push(id);
    const peer = await read.get(id);
    assert.equal(peer.comments.length, 2);
    assert.ok(peer.comments.every((c) => c.timestamp === null && Object.isFrozen(c)));
    const handle = await Issues.openAs(f.repo, f.signer).getMut(id);
    assert.deepEqual(await handle.reload(), peer);
    assert.equal((await handle.edit('Still editable')).snapshot.comments[0].timestamp, null);
    const comment = await handle.comment('Normal date after peer metadata');
    assert.equal(
      typeof comment.snapshot.comments.find((c) => c.id === comment.entryId).timestamp,
      'number',
    );
  }
  const all = await read.all();
  assert.equal(all.length, 3);
  assert.ok(Object.isFrozen(all));
  assert.deepEqual(
    all.map((i) => i.id),
    [normal.id, ...ids].sort(),
  );
  assert.deepEqual(await read.get(normal.id), before);
  assert.ok(
    all.find((i) => i.id === normal.id).comments.every((c) => typeof c.timestamp === 'number'),
  );
});

test('patch revisions, comments and reviews with unrepresentable dates stay listable and writable', async (t) => {
  const f = await fixture(t),
    base = (await f.repo.head()).oid;
  for (const seconds of ['8640000000001', '18446744073709551']) {
    const { stdout } = await promisify(execFile)(
      process.execPath,
      [fileURLToPath(new URL('./metadata-consumer.mjs', import.meta.url)), f.rid, 'patch', base],
      { env: { ...process.env, RAD_HOME: f.home, GIT_COMMITTER_DATE: seconds }, timeout: 15000 },
    );
    const id = stdout.trim(),
      read = Patches.open(f.repo);
    const p = await read.get(id);
    const r = p.revisions[0];
    for (const v of [r, ...r.comments, ...r.reviews]) assert.equal(v.timestamp, null);
    const handle = await Patches.openAs(f.repo, f.signer).getMut(id);
    const changed = await handle.label(['after-peer']);
    assert.equal(changed.snapshot.revisions[0].timestamp, null);
    assert.deepEqual((await read.get(id)).labels, ['after-peer']);
    assert.equal((await handle.reload()).revisions[0].reviews[0].timestamp, null);
    assert.ok((await read.all()).some((p) => p.id === id));
  }
});

test('commit metadata uses number-or-null independently for both dates without rejecting commit()', async (t) => {
  const f = await fixture(t);
  const tree = git(f.work, 'rev-parse', 'HEAD^{tree}');
  // Write raw peer-like objects, avoiding Git CLI date validation. No refs move.
  const stored = join(f.home, 'storage', f.rid.slice(4));
  for (const [author, committer, wantAuthor, wantCommitter] of [
    ['8640000000000', '-8640000000000', 8640000000000000, -8640000000000000],
    ['8640000000001', '1', null, 1000],
    ['-8640000000001', '8640000000001', null, null],
    ['9223372036854775807', '-9223372036854775808', null, null],
  ]) {
    const input = `tree ${tree}\nauthor Peer <peer@example.invalid> ${author} +0000\ncommitter Peer <peer@example.invalid> ${committer} +0000\n\nPeer dates\n`;
    const oid = execFileSync(
      'git',
      ['-C', stored, 'hash-object', '--literally', '-t', 'commit', '-w', '--stdin'],
      { input, encoding: 'utf8' },
    ).trim();
    const commit = await f.repo.commit(oid);
    assert.equal(commit.oid, oid);
    assert.ok(Object.isFrozen(commit));
    assert.ok(Object.hasOwn(commit, 'authorTime'));
    assert.ok(Object.hasOwn(commit, 'committerTime'));
    assert.equal(commit.authorTime, wantAuthor);
    assert.equal(commit.committerTime, wantCommitter);
    assert.equal(commit.messageStr, 'Peer dates\n');
  }
});
