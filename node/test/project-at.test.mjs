import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, mkdir, writeFile, readFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { projectAt, RadicleError } from '../dist/index.js';
import { git } from './helpers.mjs';

const rid = 'rad:z27dDy6sppkZDG8qYcySVX9Qn5jNg';
const namespace = 'z6MkgNk7k9nXpHP8vgyqCNTgSh8nAfia21QaSFyhLCSntynx';
const url = `rad://${rid.slice(4)}`;
const gitError = (e) => e instanceof RadicleError && e.code === 'GIT';

test('projectAt: explicit fetch remote, Git parents/worktrees, frozen metadata and unchanged files', async (t) => {
  const root = await mkdtemp(join(tmpdir(), 'radicle-project-at-'));
  t.after(() => rm(root, { recursive: true, force: true }));
  const work = join(root, 'work');
  await mkdir(work);
  assert.equal(await projectAt(work), null);
  git(work, 'init', '--initial-branch=main');
  await writeFile(join(work, 'file'), 'initial\n');
  git(work, 'add', '.');
  git(work, 'commit', '-m', 'initial');
  assert.equal(await projectAt(work), null);
  git(work, 'remote', 'add', 'rad', url);
  git(work, 'remote', 'add', 'peer', `${url}/${namespace}`);
  git(work, 'remote', 'set-url', '--push', 'rad', 'not-the-fetch-url');
  const nested = join(work, 'nested', 'child'),
    linked = join(root, 'linked λ');
  await mkdir(nested, { recursive: true });
  git(work, 'worktree', 'add', '-b', 'linked', linked);
  const config = await readFile(join(work, '.git/config'));
  const refs = git(work, 'for-each-ref', '--format=%(refname) %(objectname)');
  const cwd = process.cwd();
  for (const path of [work, nested, linked]) {
    const found = await projectAt(path);
    assert.equal(found.rid, rid);
    assert.equal(found.remoteName, 'rad');
    assert.equal(found.namespace, undefined);
    assert.ok(Object.isFrozen(found));
    assert.throws(() => {
      found.rid = 'wrong';
    }, TypeError);
  }
  assert.deepEqual(await projectAt(nested, { remoteName: 'peer' }), {
    rid,
    remoteName: 'peer',
    namespace,
  });
  assert.equal(process.cwd(), cwd);
  assert.deepEqual(await readFile(join(work, '.git/config')), config);
  assert.equal(git(work, 'for-each-ref', '--format=%(refname) %(objectname)'), refs);
  git(work, 'remote', 'remove', 'rad');
  assert.equal(await projectAt(work), null);
  assert.equal((await projectAt(work, { remoteName: 'peer' })).namespace, namespace);
  await assert.rejects(projectAt(join(root, 'missing')), gitError);
  for (const invalid of [
    'https://example.invalid/repo',
    'rad://invalid',
    `${url}/`,
    `${url}/bad`,
  ]) {
    git(work, 'remote', 'set-url', 'peer', invalid);
    await assert.rejects(projectAt(work, { remoteName: 'peer' }), gitError);
  }
  for (const name of ['', 'bad name', 'bad..name'])
    await assert.rejects(
      projectAt(work, { remoteName: name }),
      (e) => e instanceof TypeError && e.code === 'ERR_INVALID_ARG_VALUE',
    );
  for (const path of [undefined, null, 42, new Uint8Array()]) {
    const result = projectAt(path);
    assert.ok(result instanceof Promise);
    await assert.rejects(
      result,
      (e) => e instanceof TypeError && e.code === 'ERR_INVALID_ARG_TYPE',
    );
  }
  for (const options of [null, 42, [], 'rad', { remoteName: null }])
    await assert.rejects(projectAt(work, options), TypeError);
  await assert.rejects(projectAt(work, { remoteName: 42 }), TypeError);
});
