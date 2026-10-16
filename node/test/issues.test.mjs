import test from 'node:test';
import assert from 'node:assert/strict';
import { join } from 'node:path';
import { Issues, Profile, RadicleError } from '../dist/index.js';
import { fixture } from './helpers.mjs';

function envelope(change, id) {
  assert.match(change.entryId, /^[0-9a-f]{40}$/);
  assert.equal(change.snapshot.id, id);
  for (const value of [
    change,
    change.snapshot,
    change.snapshot.labels,
    change.snapshot.assignees,
    change.snapshot.comments,
    ...change.snapshot.comments,
  ])
    assert.ok(Object.isFrozen(value));
  assert.equal('changed' in change, false); // issue lifecycle returns an entry, not a no-op boolean
  return change.snapshot;
}
const invalid = (e) => e instanceof TypeError && e.code === 'ERR_INVALID_ARG_VALUE';
const category = (code) => (e) =>
  e instanceof RadicleError && e.code === code && e.cause instanceof Error;

test('complete issue workflow: metadata, discussion, edits, replacement sets, close/reopen and listing', async (t) => {
  const f = await fixture(t),
    read = Issues.open(f.repo),
    write = Issues.openAs(f.repo, f.signer);
  const empty = await read.all();
  assert.deepEqual(empty, []);
  assert.ok(Object.isFrozen(empty));
  const bob = await Profile.init(join(f.root, 'bob'), 'bob');
  const issue = await write.create({
    title: 'First',
    description: 'Root',
    labels: ['z', 'a', 'a'],
    assignees: [f.profile.did, bob.did, f.profile.did],
  });
  const before = await issue.reload();
  assert.equal(before.author, f.profile.did);
  assert.equal(before.closeReason, undefined);
  assert.deepEqual(before.labels, ['a', 'z']);
  assert.deepEqual(before.assignees, [f.profile.did, bob.did].sort());
  const entries = new Set();
  const change = (result) => {
    assert.ok(!entries.has(result.entryId));
    entries.add(result.entryId);
    return envelope(result, issue.id);
  };
  const other = await Issues.openAs(f.storage.repository(f.rid), f.profile.signer()).getMut(
    issue.id,
  );
  assert.equal(change(await issue.edit('Edited')).title, 'Edited');
  const rootId = before.comments[0].id;
  const discussion = await other.comment('Discussion');
  const nested = await issue.comment('Reply', { replyTo: discussion.entryId });
  assert.equal(
    change(nested).comments.find((c) => c.id === nested.entryId).replyTo,
    discussion.entryId,
  );
  const description = change(await other.editDescription('New description'));
  assert.equal(description.title, 'Edited'); // the older handle must reload, not overwrite
  assert.equal(description.description, 'New description');
  assert.equal(description.comments[0].id, rootId);
  assert.equal(description.comments[0].body, 'New description');
  assert.equal(description.comments.length, 3);
  assert.deepEqual(change(await issue.label(['only', 'only'])).labels, ['only']);
  assert.deepEqual(change(await other.label([])).labels, []);
  assert.deepEqual(change(await issue.assign([bob.did])).assignees, [bob.did]);
  assert.deepEqual(change(await other.assign([])).assignees, []);
  const closed = change(await issue.close());
  assert.equal(closed.state, 'closed');
  assert.equal(closed.closeReason, 'unspecified');
  const solved = change(await other.close({ reason: 'solved' }));
  assert.equal(solved.closeReason, 'solved');
  assert.equal(change(await issue.close({ reason: 'solved' })).closeReason, 'solved');
  const reopened = change(await other.reopen());
  assert.equal(reopened.state, 'open');
  assert.equal(reopened.closeReason, undefined);
  assert.equal(change(await issue.reopen()).state, 'open');
  assert.deepEqual(await read.get(issue.id), await issue.reload());
  await write.create({ title: 'Second', description: '' });
  await write.create({ title: 'Third', description: '' });
  const all = await read.all();
  assert.deepEqual(
    all.map((i) => i.id),
    all.map((i) => i.id).sort(),
  );
  assert.equal(all.length, 3);
  assert.ok(Object.isFrozen(all));
  assert.ok(
    all.every(
      (i) => Object.isFrozen(i) && Object.isFrozen(i.labels) && Object.isFrozen(i.assignees),
    ),
  );
  assert.deepEqual(await write.all(), all);
  assert.equal((await write.getMut(issue.id)).id, issue.id);
  assert.deepEqual(before.labels, ['a', 'z']);
  assert.equal(before.description, 'Root');
  assert.equal(before.comments.length, 1);
  assert.equal(closed.closeReason, 'unspecified');
  assert.equal(solved.state, 'closed');
  assert.equal(empty.length, 0);
  assert.throws(() => all.push(before), TypeError);
  assert.throws(() => before.labels.push('bad'), TypeError);
  assert.throws(() => before.assignees.splice(0), TypeError);
  assert.throws(() => {
    reopened.closeReason = 'solved';
  }, TypeError);
  assert.equal('issue' in issue, false); // no mutable snapshot cache
});

test('issue validation/read-only failures reject without changing persisted state; subsequent writes work', async (t) => {
  const f = await fixture(t),
    read = Issues.open(f.repo),
    write = Issues.openAs(f.repo, f.signer);
  const issue = await write.create({
    title: 'Unchanged',
    description: 'Root',
    labels: ['keep'],
    assignees: [f.profile.did],
  });
  const before = await issue.reload();
  for (const options of [
    { title: '', description: '' },
    { title: 'Bad label', description: '', labels: ['good', 'bad label'] },
    { title: 'Bad DID', description: '', assignees: [f.profile.did, 'not-a-did'] },
  ])
    await assert.rejects(write.create(options), invalid);
  for (const call of [
    () => issue.edit(''),
    () => issue.label(['valid', 'bad label']),
    () => issue.label(['']),
    () => issue.assign([f.profile.did, 'not-a-did']),
    () => issue.close({ reason: 'invalid' }),
  ])
    await assert.rejects(call(), invalid);
  for (const call of [
    () => issue.edit(42),
    () => issue.editDescription(42),
    () => issue.label('not-an-array'),
    () => issue.label([42]),
    () => issue.assign([42]),
    () => issue.close({ reason: 42 }),
    () => issue.close(null),
  ]) {
    const pending = call();
    assert.ok(pending instanceof Promise);
    await assert.rejects(
      pending,
      (e) => e instanceof TypeError && e.code === 'ERR_INVALID_ARG_TYPE',
    );
  }
  await assert.rejects(read.create({ title: 'No', description: '' }), category('READ_ONLY'));
  await assert.rejects(read.getMut(issue.id), category('READ_ONLY'));
  await assert.rejects(write.getMut('0'.repeat(40)), category('COB'));
  await assert.rejects(write.getMut('invalid'), invalid);
  await assert.rejects(read.get('invalid'), invalid);
  assert.deepEqual(await read.all(), [before]);
  assert.deepEqual(await issue.reload(), before);
  assert.equal(envelope(await issue.edit('After failure'), issue.id).title, 'After failure');
  assert.equal((await read.get(issue.id)).title, 'After failure');
});

test('mixed issue operations serialize across independent handles/signers without lost fields', async (t) => {
  const f = await fixture(t),
    write = Issues.openAs(f.repo, f.signer);
  const a = await write.create({ title: 'Before', description: 'Before' });
  const b = await Issues.openAs(f.storage.repository(f.rid), f.profile.signer()).getMut(a.id);
  const c = await Issues.openAs(f.storage.repository(f.rid), f.profile.signer()).getMut(a.id);
  const before = await a.reload();
  const results = await Promise.all([
    a.edit('Concurrent title'),
    b.editDescription('Concurrent description'),
    c.label(['b', 'a']),
    a.assign([f.profile.did]),
    b.close({ reason: 'solved' }),
    c.comment('Concurrent comment'),
  ]);
  const snapshots = results.map((r) => envelope(r, a.id));
  assert.equal(new Set(results.map((r) => r.entryId)).size, results.length);
  assert.equal(snapshots[0].title, 'Concurrent title');
  assert.equal(snapshots[1].description, 'Concurrent description');
  assert.deepEqual(snapshots[2].labels, ['a', 'b']);
  assert.deepEqual(snapshots[3].assignees, [f.profile.did]);
  assert.equal(snapshots[4].closeReason, 'solved');
  assert.ok(snapshots[5].comments.some((c) => c.id === results[5].entryId));
  const final = await a.reload();
  assert.equal(final.title, 'Concurrent title');
  assert.equal(final.description, 'Concurrent description');
  assert.deepEqual(final.labels, ['a', 'b']);
  assert.deepEqual(final.assignees, [f.profile.did]);
  assert.equal(final.state, 'closed');
  assert.equal(final.closeReason, 'solved');
  assert.equal(final.comments.length, 2);
  assert.deepEqual(await Issues.open(f.repo).all(), [final]);
  assert.equal(before.title, 'Before');
  assert.equal(before.description, 'Before');
  assert.deepEqual(before.labels, []);
  assert.deepEqual(before.assignees, []);
  assert.equal(before.state, 'open');
  assert.equal(before.comments.length, 1);
});
