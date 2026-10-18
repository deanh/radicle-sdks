import test from 'node:test';
import assert from 'node:assert/strict';
import { join } from 'node:path';
import { writeFile, unlink } from 'node:fs/promises';
import { Profile, checkoutProject, RadicleError } from 'radicle-sdk';
import { fixture, git } from './helpers.mjs';

const stored = (f) => join(f.home, 'storage', f.rid.slice(4));
const category = (code) => (e) => e instanceof RadicleError && e.code === code;
const invalid = (e) => e instanceof TypeError && e.code === 'ERR_INVALID_ARG_VALUE';
const wrongType = (e) => e instanceof TypeError && e.code === 'ERR_INVALID_ARG_TYPE';

test('repository parity: complete identity, revision parsing, tags and signed remotes', async (t) => {
  const f = await fixture(t);
  const dir = stored(f);
  const doc = await f.repo.identity();
  assert.ok(Number.isInteger(doc.version));
  assert.equal(doc.threshold, 1);
  assert.match(doc.commit, /^[0-9a-f]{40}$/);
  assert.match(doc.blob, /^[0-9a-f]{40}$/);
  assert.equal(git(dir, 'cat-file', '-t', doc.commit), 'commit');
  assert.equal(git(dir, 'cat-file', '-t', doc.blob), 'blob');
  assert.equal(await f.repo.description(), doc.description);
  assert.ok(Object.isFrozen(doc));
  const head = (await f.repo.head()).oid;
  const tree = (await f.repo.commit(head)).tree;
  const blob = (await f.repo.tree(tree)).entries[0].oid;
  git(dir, 'tag', 'z-commit', head);
  git(dir, 'tag', '-a', 'a-annotated', head, '-m', 'tag');
  git(dir, 'tag', 'tree', tree);
  git(dir, 'tag', '-a', 'blob', blob, '-m', 'blob tag');
  git(dir, 'symbolic-ref', 'refs/tags/symbolic', 'refs/tags/z-commit');
  const tags = await f.repo.tags();
  assert.deepEqual(
    tags.map((v) => v.name),
    ['a-annotated', 'blob', 'tree', 'z-commit'],
  );
  assert.deepEqual(
    tags.map((v) => v.peeledKind),
    ['commit', 'blob', 'tree', 'commit'],
  );
  assert.deepEqual(
    tags.map((v) => v.peeled),
    [head, blob, tree, head],
  );
  assert.deepEqual(
    tags.map((v) => v.kind),
    ['annotated', 'annotated', 'lightweight', 'lightweight'],
  );
  assert.notEqual(tags[0].target, tags[0].peeled);
  assert.equal(tags[3].target, tags[3].peeled);
  assert.ok(Object.isFrozen(tags));
  assert.ok(tags.every(Object.isFrozen));
  assert.equal(await f.repo.revParse('refs/tags/a-annotated'), tags[0].target);
  assert.equal(await f.repo.revParse('refs/tags/a-annotated^{}'), head);
  await assert.rejects(f.repo.revParse('does-not-exist'), category('GIT'));
  await assert.rejects(f.repo.revParse(42), wrongType);
  const signed = await f.repo.signRefs(f.signer);
  const remotes = await f.repo.remotes();
  const mine = remotes.find((r) => r.fetcher === f.profile.nodeId);
  assert.deepEqual(mine.heads, Object.keys(signed.refs).sort());
  assert.ok(Object.isFrozen(remotes));
  assert.ok(Object.isFrozen(mine.heads));
});

test('canonical writes move refs, report no-ops, need no signer and recover after failure', async (t) => {
  const f = await fixture(t);
  const dir = stored(f);
  const before = (await f.repo.head()).oid;
  assert.deepEqual(await f.repo.setDefaultBranchToCanonicalHead(), {
    old: before,
    new: before,
    isUpdated: false,
  });
  await writeFile(join(f.work, 'README.md'), 'second commit\n');
  git(f.work, 'commit', '-am', 'second');
  const next = git(f.work, 'rev-parse', 'HEAD');
  git(f.work, 'push', dir, `HEAD:refs/namespaces/${f.profile.nodeId}/refs/heads/main`);
  await f.repo.signRefs(f.signer);
  const moved = await f.repo.setDefaultBranchToCanonicalHead();
  assert.deepEqual(moved, { old: before, new: next, isUpdated: true });
  assert.ok(Object.isFrozen(moved));
  assert.equal(git(dir, 'rev-parse', 'refs/heads/main'), next);
  const results = await Promise.all(
    Array.from({ length: 6 }, () => f.storage.repository(f.rid).setDefaultBranchToCanonicalHead()),
  );
  assert.ok(results.every((r) => r.new === next && !r.isUpdated));
  const doc = await f.repo.identity();
  assert.equal(await f.repo.setIdentityHead(), doc.commit);
  assert.equal(git(dir, 'rev-parse', 'refs/rad/id'), doc.commit);
  const outsider = await Profile.init(join(f.root, 'outsider'), 'outsider');
  const root = await f.repo.setRemoteIdentityRoot(outsider.nodeId);
  assert.equal(git(dir, 'rev-parse', `refs/namespaces/${outsider.nodeId}/refs/rad/root`), root);
  assert.equal(await f.repo.setRemoteIdentityRoot(outsider.nodeId), root);
  await assert.rejects(f.repo.setRemoteIdentityRoot('invalid'), invalid);
  await assert.rejects(f.repo.setRemoteIdentityRoot(42), wrongType);
  const lock = join(dir, 'refs', 'rad', 'id.lock');
  await writeFile(lock, 'locked');
  await assert.rejects(f.repo.setIdentityHead(), category('REPOSITORY'));
  await unlink(lock);
  assert.equal(await f.repo.setIdentityHead(), doc.commit);
});

test('bare checkout is opt-in and options are not coerced', async (t) => {
  const f = await fixture(t);
  for (const bare of [false, true]) {
    const path = join(f.root, bare ? 'bare' : 'working');
    assert.equal(await checkoutProject(f.storage, f.rid, f.profile.nodeId, path, { bare }), path);
    assert.equal(git(path, 'rev-parse', '--is-bare-repository'), String(bare));
    assert.equal(git(path, 'rev-parse', 'HEAD'), (await f.repo.head()).oid);
  }
  for (const options of [null, [], 1, { bare: null }, { bare: 'true' }]) {
    await assert.rejects(
      checkoutProject(f.storage, f.rid, f.profile.nodeId, join(f.root, 'invalid'), options),
      wrongType,
    );
  }
  const missing = (await Profile.init(join(f.root, 'empty'), 'empty')).storage.repository(f.rid);
  for (const method of [
    'description',
    'tags',
    'remotes',
    'setIdentityHead',
    'setDefaultBranchToCanonicalHead',
  ]) {
    await assert.rejects(missing[method](), category('REPOSITORY'));
  }
});
