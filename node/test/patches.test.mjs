import test from 'node:test';
import assert from 'node:assert/strict';
import { join } from 'node:path';
import { writeFile } from 'node:fs/promises';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { fileURLToPath } from 'node:url';
import { Profile, Patches, RadicleError } from '../dist/index.js';
import { fixture, git } from './helpers.mjs';

const stored = f => join(f.home, 'storage', f.rid.slice(4));
async function addCommit(f, text, nodeId = f.profile.nodeId, branch = 'main') {
  await writeFile(join(f.work, 'README.md'), text);
  git(f.work, 'commit', '-am', text.trim());
  const head = git(f.work, 'rev-parse', 'HEAD');
  // Import existing Git commits, not an SDK commit-import API or interactive CLI.
  git(f.work, 'push', stored(f), `HEAD:refs/namespaces/${nodeId}/refs/heads/${branch}`);
  return head;
}
function frozen(value) {
  if (!value || typeof value !== 'object') return;
  assert.ok(Object.isFrozen(value));
  for (const v of Object.values(value)) frozen(v);
}
function entry(change, id) {
  frozen(change);
  assert.match(change.entryId, /^[0-9a-f]{40}$/);
  assert.equal(change.snapshot.id, id);
  assert.equal('changed' in change, false);
  return change.snapshot;
}
function revision(p, id) { return p.revisions.find(r => r.id === id); }
function cobTip(f, id, nodeId = f.profile.nodeId) {
  return git(stored(f), 'rev-parse', `refs/namespaces/${nodeId}/refs/cobs/xyz.radicle.patch/${id}`);
}
const invalid = e => e instanceof TypeError && e.code === 'ERR_INVALID_ARG_VALUE';
const category = code => e => e instanceof RadicleError && e.code === code && e.cause instanceof Error;

test('patch workflow: draft/ready, metadata, revision, discussion/review, merge record and signed refs', async t => {
  const f = await fixture(t), base = (await f.repo.head()).oid;
  const head = await addCommit(f, 'first patch\n');
  const store = Patches.openAs(f.repo, f.signer), read = Patches.open(f.repo);
  assert.deepEqual(await read.all(), []);
  const bob = await Profile.init(join(f.root, 'bob'), 'bob');
  const p = await store.draft({ title: 'Change', description: 'First', base, head, labels: ['z', 'a', 'a'] });
  const before = await p.reload();
  frozen(before);
  assert.equal(before.state, 'draft');
  assert.equal(before.author, f.profile.did);
  assert.equal(before.target, 'delegates');
  assert.deepEqual(before.labels, ['a', 'z']);
  assert.deepEqual(before.assignees, []);
  assert.equal(before.version, 0);
  assert.equal(before.revisions.length, 1);
  assert.equal(revision(before, before.latestRevision).head, head);
  assert.equal(typeof before.revisions[0].timestamp, 'number');
  assert.equal('patch' in p, false);
  const ready = await p.ready();
  frozen(ready);
  assert.equal(ready.changed, true);
  assert.equal(ready.snapshot.state, 'open');
  const tip = cobTip(f, p.id);
  const noReady = await p.ready();
  assert.equal(noReady.changed, false);
  assert.equal(cobTip(f, p.id), tip); // no transaction for upstream no-op
  assert.equal((await p.unready()).snapshot.state, 'draft');
  const draftTip = cobTip(f, p.id);
  assert.equal((await p.unready()).changed, false);
  assert.equal(cobTip(f, p.id), draftTip);
  await p.ready();
  assert.equal(entry(await p.edit({ title: 'Edited', target: 'branch:refs/heads/main' }), p.id).target, 'branch:main');
  await p.edit({ title: 'Edited', target: 'delegates' });
  assert.deepEqual(entry(await p.label(['only', 'only']), p.id).labels, ['only']);
  assert.deepEqual(entry(await p.label([]), p.id).labels, []);
  assert.deepEqual(entry(await p.assign([bob.did, bob.did]), p.id).assignees, [bob.did]);
  assert.deepEqual(entry(await p.assign([]), p.id).assignees, []);
  assert.equal((await p.archive()).snapshot.state, 'archived');
  const archiveTip = cobTip(f, p.id);
  assert.equal((await p.archive()).changed, true); // upstream always records archive
  assert.notEqual(cobTip(f, p.id), archiveTip);
  assert.equal((await p.unarchive()).snapshot.state, 'open');
  const openTip = cobTip(f, p.id);
  assert.equal((await p.unarchive()).changed, false);
  assert.equal(cobTip(f, p.id), openTip);
  const head2 = await addCommit(f, 'second revision\n');
  const updated = await p.update({ description: 'Second', base, head: head2 });
  frozen(updated);
  assert.match(updated.revisionId, /^[0-9a-f]{40}$/);
  assert.equal('entryId' in updated, false);
  assert.equal(updated.snapshot.latestRevision, updated.revisionId);
  assert.equal(updated.snapshot.version, 1);
  assert.equal(updated.snapshot.description, 'Second');
  assert.equal(updated.snapshot.head, head2);
  const peer = await Patches.openAs(f.storage.repository(f.rid), bob.signer()).getMut(p.id);
  const comment = await peer.comment(updated.revisionId, 'Looks good');
  const reply = await p.comment(updated.revisionId, 'Reply', { replyTo: comment.entryId });
  const discussion = revision(entry(reply, p.id), updated.revisionId).comments;
  assert.equal(discussion.length, 2);
  assert.equal(discussion[1].replyTo, comment.entryId);
  assert.equal(discussion[0].author, bob.did);
  const review = await peer.review(updated.revisionId, { verdict: 'accept', summary: 'Approved' });
  const reviews = revision(entry(review, p.id), updated.revisionId).reviews;
  assert.equal(reviews[0].id, review.entryId);
  assert.equal(reviews[0].author, bob.did);
  assert.equal(reviews[0].verdict, 'accept');
  assert.equal(reviews[0].summary, 'Approved');
  const branches = () => git(stored(f), 'for-each-ref', '--format=%(refname) %(objectname)', 'refs/heads', `refs/namespaces/${f.profile.nodeId}/refs/heads`);
  const branchRefs = branches(), workHead = git(f.work, 'rev-parse', 'HEAD');
  const merged = entry(await p.merge(updated.revisionId, head2), p.id);
  assert.equal(merged.state, 'merged');
  assert.equal(merged.mergedRevision, updated.revisionId);
  assert.equal(merged.mergedCommit, head2);
  assert.equal(branches(), branchRefs);
  assert.equal(git(f.work, 'rev-parse', 'HEAD'), workHead); // merge() never merges Git
  const mergedTip = cobTip(f, p.id);
  assert.equal((await p.unready()).changed, false);
  assert.equal(cobTip(f, p.id), mergedTip);
  assert.deepEqual(await p.reload(), merged);
  assert.deepEqual(await read.get(p.id), merged);
  const all = await read.all();
  frozen(all);
  assert.deepEqual(all, [merged]);
  assert.equal(await read.get('0'.repeat(40)), null);
  const sigrefs = git(stored(f), 'show', `refs/namespaces/${f.profile.nodeId}/refs/rad/sigrefs:refs`);
  assert.ok(sigrefs.split('\n').some(line => line.trim() === `${mergedTip} refs/cobs/xyz.radicle.patch/${p.id}`));
  assert.equal(before.state, 'draft');
  assert.equal(before.revisions.length, 1);
  assert.deepEqual(before.labels, ['a', 'z']);
  assert.equal(updated.snapshot.revisions[1].comments.length, 0);
  assert.throws(() => { reviews[0].summary = 'changed'; }, TypeError);
});

test('patch validation, readonly stores and authorization fail without poisoning later operations', async t => {
  const f = await fixture(t), base = (await f.repo.head()).oid;
  const head = await addCommit(f, 'patch\n');
  const store = Patches.openAs(f.repo, f.signer), read = Patches.open(f.repo);
  const p = await store.create({ title: 'Title', description: 'Description', base, head });
  const before = await p.reload(), rev = before.latestRevision;
  const tip = cobTip(f, p.id);
  for (const call of [
    () => store.create({ title: '', description: '', base, head }),
    () => store.draft({ title: 'x', description: '', base: 'bad', head }),
    () => store.create({ title: 'x', description: '', base, head, labels: ['bad label'] }),
    () => store.create({ title: 'x', description: '', base, head, target: 'invalid' }),
    () => p.edit({ title: '', target: 'delegates' }), () => p.edit({ title: 'x', target: 'branch:bad..branch' }),
    () => p.label(['bad label']), () => p.assign(['not-a-did']),
    () => p.update({ description: '', base, head: 'bad' }),
    () => p.comment('bad', 'x'), () => p.comment(rev, 'x', { replyTo: 'bad' }),
    () => p.review(rev, { verdict: 'invalid' }), () => p.merge(rev, 'bad'),
  ]) await assert.rejects(call(), invalid);
  for (const call of [() => p.edit(null), () => p.update(null), () => p.label([42]),
    () => p.review(rev, null), () => p.review(rev, { verdict: 42 }), () => p.comment(rev, 'x', null)]) {
    const pending = call();
    assert.ok(pending instanceof Promise);
    await assert.rejects(pending, e => e instanceof TypeError && e.code === 'ERR_INVALID_ARG_TYPE');
  }
  assert.equal(cobTip(f, p.id), tip);
  assert.deepEqual(await p.reload(), before);
  for (const call of [() => read.create({ title: 'No', description: '', base, head }),
    () => read.draft({ title: 'No', description: '', base, head }), () => read.getMut(p.id)])
    await assert.rejects(call(), category('READ_ONLY'));
  await assert.rejects(store.getMut('0'.repeat(40)), category('COB'));
  await assert.rejects(read.get('bad'), invalid);
  await assert.rejects(store.getMut('bad'), invalid);
  const bob = await Profile.init(join(f.root, 'bob'), 'bob');
  const peer = await Patches.openAs(f.repo, bob.signer()).getMut(p.id);
  for (const call of [() => peer.edit({ title: 'Unauthorized', target: 'delegates' }),
    () => peer.assign([bob.did]), () => peer.merge(rev, head)]) await assert.rejects(call(), category('COB'));
  for (const call of [() => p.review(rev), () => p.comment('0'.repeat(40), 'missing'),
    () => p.comment(rev, 'missing reply', { replyTo: 'f'.repeat(40) }),
    () => p.update({ description: 'missing commit', base, head: '0'.repeat(40) }),
    () => p.merge(rev, '0'.repeat(40))]) await assert.rejects(call(), category('COB'));
  assert.equal((await p.reload()).title, 'Title');
  const summary = entry(await peer.review(rev, { summary: 'Comment-only review' }), p.id);
  assert.equal(revision(summary, rev).reviews[0].verdict, undefined);
  const repeated = await peer.review(rev, { verdict: 'reject' });
  const unchangedReview = revision(entry(repeated, p.id), rev).reviews[0];
  assert.equal(unchangedReview.verdict, undefined); // upstream first review per actor wins
  assert.notEqual(unchangedReview.id, repeated.entryId); // operation recorded, action ignored
  const reject = entry(await p.review(rev, { verdict: 'reject' }), p.id);
  assert.equal(revision(reject, rev).reviews.find(r => r.author === f.profile.did).verdict, 'reject');
  assert.equal(entry(await p.label(['after-failure']), p.id).labels[0], 'after-failure');
  // A merge record may be accepted but ignored: commit is not on target branch.
  const offTarget = await addCommit(f, 'off target\n', f.profile.nodeId, 'feature');
  const ignored = entry(await p.merge(rev, offTarget), p.id);
  assert.equal(ignored.state, 'open');
  assert.equal(ignored.mergedCommit, undefined);
});

test('latestRevision follows the patch author, not the last revision or version index; upstream branch parsing', async t => {
  const f = await fixture(t), base = (await f.repo.head()).oid;
  const store = Patches.openAs(f.repo, f.signer);
  const p = await store.create({ title: 'Empty diff allowed', description: 'Author revision', base, head: base });
  const before = await p.reload();
  const bob = await Profile.init(join(f.root, 'bob'), 'bob');
  const peerHead = await addCommit(f, 'peer revision\n', bob.nodeId);
  const peer = await Patches.openAs(f.repo, bob.signer()).getMut(p.id);
  const update = await peer.update({ description: 'Peer revision', base, head: peerHead });
  assert.equal(update.snapshot.version, 1);
  assert.equal(update.snapshot.revisions[1].id, update.revisionId);
  assert.equal(update.snapshot.revisions[1].author, bob.did);
  assert.equal(update.snapshot.latestRevision, before.latestRevision);
  assert.equal(update.snapshot.head, base);
  assert.equal(update.snapshot.description, 'Author revision');
  const latest = revision(update.snapshot, update.snapshot.latestRevision);
  assert.equal(latest.base, update.snapshot.base);
  assert.notEqual(update.snapshot.revisions[update.snapshot.version].id, update.snapshot.latestRevision);
  for (const [target, expected] of [
    ['branch:main', 'branch:main'], ['branch:refs/heads/main', 'branch:main'],
    ['branch:refs/heads/refs/heads/nested', 'branch:refs/heads/nested'],
  ]) assert.equal(entry(await p.edit({ title: 'Target', target }), p.id).target, expected);
  const another = await store.draft({ title: 'Another', description: '', target: 'branch:refs/heads/main', base, head: base });
  assert.equal((await another.reload()).target, 'branch:main');
  const all = await store.all();
  assert.deepEqual(all.map(p => p.id), [p.id, another.id].sort());
  assert.equal(before.revisions.length, 1);
});

test('mixed patch writes across handles/signers return their own snapshots without lost fields', async t => {
  const f = await fixture(t), base = (await f.repo.head()).oid;
  const head = await addCommit(f, 'first\n');
  const store = Patches.openAs(f.repo, f.signer);
  const a = await store.create({ title: 'Before', description: 'Before', base, head });
  const b = await Patches.openAs(f.storage.repository(f.rid), f.profile.signer()).getMut(a.id);
  const c = await Patches.openAs(f.storage.repository(f.rid), f.profile.signer()).getMut(a.id);
  const before = await a.reload(), rev = before.latestRevision;
  const head2 = await addCommit(f, 'second\n');
  const results = await Promise.all([
    a.update({ description: 'Concurrent revision', base, head: head2 }), b.label(['b', 'a']),
    c.assign([f.profile.did]), a.edit({ title: 'Concurrent title', target: 'delegates' }),
    b.comment(rev, 'Concurrent comment'), c.review(rev, { verdict: 'accept', summary: 'Concurrent review' }),
  ]);
  for (const r of results) frozen(r);
  assert.ok(revision(results[0].snapshot, results[0].revisionId));
  assert.equal(results[0].snapshot.head, head2);
  assert.deepEqual(results[1].snapshot.labels, ['a', 'b']);
  assert.deepEqual(results[2].snapshot.assignees, [f.profile.did]);
  assert.equal(results[3].snapshot.title, 'Concurrent title');
  assert.ok(revision(results[4].snapshot, rev).comments.some(c => c.id === results[4].entryId));
  assert.ok(revision(results[5].snapshot, rev).reviews.some(r => r.id === results[5].entryId));
  const final = await a.reload();
  assert.equal(final.title, 'Concurrent title');
  assert.equal(final.latestRevision, results[0].revisionId);
  assert.equal(final.description, 'Concurrent revision');
  assert.deepEqual(final.labels, ['a', 'b']);
  assert.deepEqual(final.assignees, [f.profile.did]);
  assert.equal(revision(final, rev).comments.length, 1);
  assert.equal(revision(final, rev).reviews.length, 1);
  assert.equal(before.title, 'Before');
  assert.equal(before.revisions.length, 1);
});

test('patch comments serialize across three independently loaded worker realms', { timeout: 60000 }, async t => {
  const f = await fixture(t), base = (await f.repo.head()).oid;
  const p = await Patches.openAs(f.repo, f.signer).create({ title: 'Workers', description: '', base, head: base });
  const rev = (await p.reload()).latestRevision;
  const { stdout } = await promisify(execFile)(process.execPath, [
    fileURLToPath(new URL('./patch-workers.mjs', import.meta.url)), f.rid, p.id, rev,
  ], { env: { ...process.env, RAD_HOME: f.home }, timeout: 50000, maxBuffer: 1024 * 1024 });
  const results = JSON.parse(stdout);
  assert.equal(results.length, 12);
  assert.equal(new Set(results.map(r => r.entryId)).size, 12);
  assert.deepEqual(results.map(r => revision(r.snapshot, rev).comments.length).sort((a,b) => a-b), Array.from({ length: 12 }, (_, i) => i + 1));
  for (const result of results) assert.ok(revision(result.snapshot, rev).comments.some(c => c.id === result.entryId));
  assert.equal(revision(await p.reload(), rev).comments.length, 12);
});
