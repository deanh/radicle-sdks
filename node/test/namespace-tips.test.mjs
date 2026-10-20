import assert from 'node:assert/strict';
import { test } from 'node:test';
import { writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import { fixture, git } from './helpers.mjs';

test('namespaceTips lists immutable ref targets without loading signed sets or objects', async (t) => {
  const { repo, profile, storage, rid } = await fixture(t);
  const stored = join(storage.path, rid.slice(4));
  const ref = `refs/namespaces/${profile.nodeId}/refs/rad/sigrefs`;
  const before = git(stored, 'for-each-ref', '--format=%(refname) %(objectname)');
  const tips = await repo.namespaceTips();
  assert.ok(Object.isFrozen(tips));
  assert.ok(Object.isFrozen(tips[0]));
  assert.deepEqual(tips, [{ nodeId: profile.nodeId, target: git(stored, 'rev-parse', ref) }]);
  assert.deepEqual(
    tips.map((tip) => tip.nodeId),
    (await repo.remotes()).map((r) => r.fetcher),
  );
  assert.equal(git(stored, 'for-each-ref', '--format=%(refname) %(objectname)'), before);
  const blob = git(stored, 'rev-parse', 'HEAD:README.md');
  git(stored, 'update-ref', ref, blob);
  assert.equal((await repo.namespaceTips())[0].target, blob);
  await assert.rejects(repo.remotes(), (e) => e.code === 'GIT');
  await writeFile(join(stored, ref), `${'1'.repeat(40)}\n`);
  assert.equal((await repo.namespaceTips())[0].target, '1'.repeat(40));
  await assert.rejects(repo.remotes(), (e) => e.code === 'GIT');
});

test('namespaceTips resolves symbolic refs and rejects invalid namespaces and dangling refs', async (t) => {
  const { repo, profile, storage, rid } = await fixture(t);
  const stored = join(storage.path, rid.slice(4));
  const ref = `refs/namespaces/${profile.nodeId}/refs/rad/sigrefs`;
  const original = git(stored, 'rev-parse', ref);
  git(stored, 'update-ref', 'refs/test/sigrefs-target', original);
  git(stored, 'symbolic-ref', ref, 'refs/test/sigrefs-target');
  assert.equal((await repo.namespaceTips())[0].target, original);
  git(stored, 'symbolic-ref', ref, 'refs/test/missing');
  await assert.rejects(repo.namespaceTips(), (e) => e.code === 'GIT');
  git(stored, 'symbolic-ref', '--delete', ref);
  assert.deepEqual(await repo.namespaceTips(), []);
  git(stored, 'update-ref', 'refs/namespaces/not-a-node/refs/rad/sigrefs', original);
  await assert.rejects(repo.namespaceTips(), (e) => e.code === 'GIT');
});
