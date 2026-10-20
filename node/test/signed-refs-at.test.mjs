import assert from 'node:assert/strict';
import { test } from 'node:test';
import { join } from 'node:path';
import { Issues, Profile } from '../dist/index.js';
import { fixture, git } from './helpers.mjs';

test('signedRefsAt loads historical sets without consulting current refs', async (t) => {
  const { repo, profile, signer, storage, rid } = await fixture(t);
  const stored = join(storage.path, rid.slice(4));
  const old = await repo.signRefs(signer);
  const loaded = await repo.signedRefsAt(profile.nodeId, old.at);
  assert.deepEqual(loaded.refs, old.refs);
  assert.equal(loaded.at, old.at);
  assert.equal(loaded.featureLevel, 'parent'); // verified root promotion upstream
  assert.ok(Object.isFrozen(loaded));
  assert.ok(Object.isFrozen(loaded.refs));
  const issue = await Issues.openAs(repo, signer).create({ title: 'signed', description: 'root' });
  const current = await repo.signRefs(signer);
  const before = git(stored, 'for-each-ref', '--format=%(refname) %(objectname)');
  assert.deepEqual(await repo.signedRefsAt(profile.nodeId, old.at), loaded);
  assert.deepEqual(await repo.signedRefsAt(profile.nodeId, current.at), current);
  assert.equal(current.parent, old.at);
  assert.equal(current.refs[`refs/cobs/xyz.radicle.issue/${issue.id}`], issue.id);
  assert.equal(git(stored, 'for-each-ref', '--format=%(refname) %(objectname)'), before);
  git(stored, 'update-ref', '-d', `refs/namespaces/${profile.nodeId}/refs/rad/sigrefs`);
  assert.deepEqual(await repo.signedRefsAt(profile.nodeId, old.at), loaded);
  git(stored, 'reflog', 'expire', '--expire=now', '--all');
  git(stored, 'gc', '--prune=now');
  await assert.rejects(
    repo.signedRefsAt(profile.nodeId, old.at),
    (e) => e.code === 'GIT' && e.message.includes(old.at),
  );
});

test('signedRefsAt preserves upstream replay protection and error classes', async (t) => {
  const { repo, profile, signer, root, storage, rid } = await fixture(t);
  const stored = join(storage.path, rid.slice(4));
  const signed = await repo.signRefs(signer);
  const peer = await Profile.init(join(root, 'peer'), 'peer');
  const rejectsGit = (nid, oid) =>
    assert.rejects(repo.signedRefsAt(nid, oid), (e) => e.code === 'GIT' && e.message.includes(oid));
  await assert.rejects(
    repo.signedRefsAt('bad', signed.at),
    (e) => e.code === 'ERR_INVALID_ARG_VALUE',
  );
  await assert.rejects(
    repo.signedRefsAt(profile.nodeId, 'bad'),
    (e) => e.code === 'ERR_INVALID_ARG_VALUE',
  );
  await rejectsGit(profile.nodeId, '1'.repeat(40));
  await rejectsGit(profile.nodeId, git(stored, 'rev-parse', 'HEAD:README.md'));
  await rejectsGit(peer.nodeId, signed.at);
  const tree = git(stored, 'rev-parse', `${signed.at}^{tree}`);
  const replay = git(stored, 'commit-tree', tree, '-p', signed.at, '-m', 'replay');
  assert.equal((await repo.signedRefsAt(profile.nodeId, replay)).at, signed.at);
  await Issues.openAs(repo, signer).create({ title: 'modern', description: 'root' });
  const modern = await repo.signRefs(signer);
  const modernTree = git(stored, 'rev-parse', `${modern.at}^{tree}`);
  const invalid = git(stored, 'commit-tree', modernTree, '-p', modern.at, '-m', 'invalid parent');
  await rejectsGit(profile.nodeId, invalid);
});
