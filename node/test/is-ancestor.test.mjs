import assert from 'node:assert/strict';
import { test } from 'node:test';
import { unlink } from 'node:fs/promises';
import { join } from 'node:path';
import { fixture, git } from './helpers.mjs';

test('isAncestor is reflexive plain-Git ancestry over branches and merges', async (t) => {
  const { repo, storage, rid } = await fixture(t);
  const stored = join(storage.path, rid.slice(4));
  const root = git(stored, 'rev-parse', 'HEAD');
  const tree = git(stored, 'rev-parse', 'HEAD^{tree}');
  const left = git(stored, 'commit-tree', tree, '-p', root, '-m', 'left');
  const right = git(stored, 'commit-tree', tree, '-p', root, '-m', 'right');
  const merge = git(stored, 'commit-tree', tree, '-p', left, '-p', right, '-m', 'merge');
  const unrelated = git(stored, 'commit-tree', tree, '-m', 'unrelated');
  const before = git(stored, 'for-each-ref', '--format=%(refname) %(objectname)');
  for (const [ancestor, descendant, expected] of [
    [root, root, true],
    [root, merge, true],
    [left, merge, true],
    [right, merge, true],
    [merge, root, false],
    [left, right, false],
    [unrelated, merge, false],
  ])
    assert.equal(await repo.isAncestor(ancestor, descendant), expected);
  assert.equal(git(stored, 'for-each-ref', '--format=%(refname) %(objectname)'), before);
  const blob = git(stored, 'rev-parse', 'HEAD:README.md');
  const missing = '1'.repeat(40);
  for (const [a, d] of [
    [missing, root],
    [root, missing],
    [missing, missing],
    [blob, blob],
    [root, blob],
  ])
    await assert.rejects(repo.isAncestor(a, d), (e) => e.code === 'GIT');
  await assert.rejects(repo.isAncestor('bad', root), (e) => e.code === 'ERR_INVALID_ARG_VALUE');
  await assert.rejects(repo.isAncestor(root, null), (e) => e.code === 'ERR_INVALID_ARG_TYPE');
});

for (const cached of [false, true])
  test(`isAncestor propagates missing-intermediate errors, commit-graph=${cached}`, async (t) => {
    const { repo, storage, rid } = await fixture(t);
    const stored = join(storage.path, rid.slice(4));
    const root = git(stored, 'rev-parse', 'HEAD');
    const tree = git(stored, 'rev-parse', 'HEAD^{tree}');
    const middle = git(stored, 'commit-tree', tree, '-p', root, '-m', 'middle');
    const tip = git(stored, 'commit-tree', tree, '-p', middle, '-m', 'tip');
    git(stored, 'update-ref', 'refs/heads/graph-query', tip);
    if (cached) git(stored, 'commit-graph', 'write', '--reachable');
    assert.equal(await repo.isAncestor(root, tip), true);
    await unlink(join(stored, 'objects', middle.slice(0, 2), middle.slice(2)));
    await assert.rejects(repo.isAncestor(root, tip), (e) => e.code === 'GIT');
  });
