import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdir, writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import { initProject, forkProject, checkoutProject, Issues } from 'radicle-sdk';
import { fixture, git } from './helpers.mjs';

async function workingCopy(root, name) {
  const work = join(root, name);
  await mkdir(work);
  git(work, 'init', '--initial-branch=main');
  await writeFile(join(work, 'README.md'), '# Visibility\n');
  git(work, 'add', '.'); git(work, 'commit', '-m', 'initial');
  return work;
}
const options = { name: 'visibility', description: 'Local identity', defaultBranch: 'main' };

test('initProject defaults public; explicit public/private identities support local signed workflows', async t => {
  const f = await fixture(t);
  assert.equal((await f.repo.identity()).visibility, 'public');
  for (const visibility of ['public', 'private']) {
    const work = await workingCopy(f.root, visibility);
    const rid = await initProject(f.storage, f.signer, work, { ...options, visibility });
    const repo = f.storage.repository(rid);
    assert.equal((await repo.identity()).visibility, visibility);
    assert.equal((await repo.head()).oid, git(work, 'rev-parse', 'HEAD'));
    assert.equal(await f.storage.contains(rid), true);
    const issue = await Issues.openAs(repo, f.signer).create({ title: visibility, description: 'Root' });
    assert.equal((await issue.comment('Signed locally')).snapshot.comments.length, 2);
    await forkProject(f.storage, f.signer, rid);
    const checkout = join(f.root, `${visibility}-checkout`);
    await checkoutProject(f.storage, rid, f.signer.nodeId, checkout);
    assert.equal(git(checkout, 'rev-parse', 'HEAD'), (await repo.head()).oid);
    assert.equal((await repo.identity()).visibility, visibility);
  }
});

test('invalid init visibility rejects before storage/remote side effects; a valid retry succeeds', async t => {
  const f = await fixture(t);
  const work = await workingCopy(f.root, 'invalid');
  const before = await f.storage.repositories();
  for (const visibility of ['', 'Private', 'internal', 42, false, {}, null]) {
    await assert.rejects(initProject(f.storage, f.signer, work, { ...options, visibility }), e =>
      e instanceof TypeError && e.code === (typeof visibility === 'string' ? 'ERR_INVALID_ARG_VALUE' : 'ERR_INVALID_ARG_TYPE'));
    assert.deepEqual(await f.storage.repositories(), before);
    assert.equal(git(work, 'remote'), '');
  }
  const rid = await initProject(f.storage, f.signer, work, { ...options, visibility: 'private' });
  assert.equal((await f.storage.repository(rid).identity()).visibility, 'private');
  const remote = git(work, 'config', '--get-regexp', '^remote.rad.');
  await assert.rejects(initProject(f.storage, f.signer, work, { ...options, visibility: 'public' }), e => e.code === 'REPOSITORY');
  assert.equal(git(work, 'config', '--get-regexp', '^remote.rad.'), remote);
  assert.equal((await f.storage.repository(rid).identity()).visibility, 'private');
});
