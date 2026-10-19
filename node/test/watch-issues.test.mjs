import test from 'node:test';
import assert from 'node:assert/strict';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { readFile, writeFile, access, symlink, mkdir, readdir, rename, rm } from 'node:fs/promises';
import { Issues } from 'radicle-sdk';
import { fixture, git } from './helpers.mjs';
import { pollOnce } from '../../examples/watch-issues.mjs';
const script = fileURLToPath(new URL('../../examples/watch-issues.mjs', import.meta.url));
const run = promisify(execFile);
const refs = (f) =>
  git(join(f.home, 'storage', f.rid.slice(4)), 'for-each-ref', '--format=%(refname) %(objectname)');
async function cli(f, state, ...extra) {
  const { stdout } = await run(
    process.execPath,
    [script, f.rid, '--state', state, '--once', ...extra],
    { cwd: f.root, env: { ...process.env, RAD_HOME: f.home }, timeout: 60000 },
  );
  return stdout.trim() ? stdout.trim().split('\n').map(JSON.parse) : [];
}

test('observer CLI baselines, emits typed discovery/comments, checkpoints and restarts without writing refs', async (t) => {
  const f = await fixture(t),
    state = join(f.root, 'checkpoint.json');
  const issues = Issues.openAs(f.repo, f.signer);
  const old = await issues.create({ title: 'old', description: 'root' });
  await old.comment('before');
  let before = refs(f);
  assert.deepEqual(await cli(f, state), []);
  assert.equal(refs(f), before);
  const fresh = await issues.create({ title: 'new', description: 'not another event' });
  const first = (await fresh.comment('snowman ☃\n"quoted"')).entryId;
  const second = (await old.comment('after')).entryId;
  before = refs(f);
  const events = await cli(f, state);
  assert.deepEqual(
    events.map((e) => e.id).sort(),
    [
      `${f.rid}/issue/${fresh.id}`,
      `${f.rid}/issue/${fresh.id}/comment/${first}`,
      `${f.rid}/issue/${old.id}/comment/${second}`,
    ].sort(),
  );
  assert.deepEqual(
    events.map((e) => e.issueId),
    events.map((e) => e.issueId).sort(),
  );
  assert.equal(events.filter((e) => e.type === 'issue.discovered').length, 1);
  const saved = await readFile(state, 'utf8');
  assert.deepEqual(await cli(f, state, '--replay'), []);
  assert.equal(await readFile(state, 'utf8'), saved);
  assert.equal(refs(f), before);
});

test('awaited callback failure leaves the old checkpoint; successful earlier callbacks may repeat', async (t) => {
  const f = await fixture(t),
    state = join(f.root, 'checkpoint.json');
  const issue = await Issues.openAs(f.repo, f.signer).create({
    title: 'existing',
    description: 'root',
  });
  await issue.comment('comment');
  const partial = [];
  await assert.rejects(
    pollOnce(
      f.profile,
      f.rid,
      state,
      async (event) => {
        await Promise.resolve();
        partial.push(event);
        if (partial.length === 2) throw new Error('handler failed');
      },
      { replay: true },
    ),
    /handler failed/,
  );
  await assert.rejects(access(state));
  const delivered = [];
  assert.equal(
    await pollOnce(
      f.profile,
      f.rid,
      state,
      async (event) => {
        delivered.push(event);
      },
      { replay: true },
    ),
    2,
  );
  assert.deepEqual(delivered, partial);
  const before = await readFile(state, 'utf8');
  await issue.comment('later');
  await assert.rejects(
    pollOnce(f.profile, f.rid, state, () => {
      throw new Error('no advance');
    }),
    /no advance/,
  );
  assert.equal(await readFile(state, 'utf8'), before);
  assert.equal(await pollOnce(f.profile, f.rid, state, () => {}), 1);
});

test('checkpoint replace failure cleans temporary files and does not acknowledge delivered events', async (t) => {
  const f = await fixture(t),
    state = join(f.root, 'checkpoint.json');
  const issue = await Issues.openAs(f.repo, f.signer).create({ title: 't', description: 'root' });
  await pollOnce(f.profile, f.rid, state, () => {});
  const before = await readFile(state, 'utf8');
  await issue.comment('after');
  const delivered = [];
  // The callback can change the filesystem after path validation. An obstructing
  // directory forces rename to fail without requiring a private example hook.
  const moved = `${state}.saved`;
  await assert.rejects(
    pollOnce(f.profile, f.rid, state, async (event) => {
      delivered.push(event);
      await rename(state, moved);
      await mkdir(state);
    }),
  );
  assert.equal(await readFile(moved, 'utf8'), before);
  assert.ok(!(await readdir(f.root)).some((name) => name.startsWith('.checkpoint.json.')));
  await rm(state, { recursive: true });
  await rename(moved, state);
  const again = [];
  await pollOnce(f.profile, f.rid, state, (event) => again.push(event));
  assert.deepEqual(again, delivered);
});

test('invalid checkpoint scope/shape and protected paths fail closed', async (t) => {
  const f = await fixture(t),
    state = join(f.root, 'checkpoint.json');
  await pollOnce(f.profile, f.rid, state, () => {});
  const good = JSON.parse(await readFile(state, 'utf8'));
  for (const change of [
    { version: 2 },
    { version: true },
    { storage: 'wrong' },
    { rid: 'wrong' },
    { seen: { bad: [] } },
    { seen: { ['0'.repeat(40)]: ['invalid'] } },
    { seen: { ['0'.repeat(40)]: ['1'.repeat(40), '1'.repeat(40)] } },
    { unexpected: 1 },
  ]) {
    const bytes = JSON.stringify({ ...good, ...change });
    await writeFile(state, bytes);
    await assert.rejects(pollOnce(f.profile, f.rid, state, () => {}));
    assert.equal(await readFile(state, 'utf8'), bytes);
  }
  await writeFile(state, '{bad JSON');
  await assert.rejects(cli(f, state));
  assert.equal(await readFile(state, 'utf8'), '{bad JSON');
  for (const path of [join(f.home, 'checkpoint'), join(f.profile.storage.path, 'checkpoint')]) {
    await assert.rejects(
      pollOnce(f.profile, f.rid, path, () => {}),
      /outside/,
    );
    await assert.rejects(access(path));
  }
  const target = join(f.root, 'target');
  await writeFile(target, 'do not overwrite');
  const link = join(f.root, 'link');
  await symlink(target, link);
  await assert.rejects(
    pollOnce(f.profile, f.rid, link, () => {}),
    /symlink/,
  );
  assert.equal(await readFile(target, 'utf8'), 'do not overwrite');
  const parent = join(f.root, 'home-link');
  await symlink(f.home, parent);
  await assert.rejects(
    pollOnce(f.profile, f.rid, join(parent, 'checkpoint'), () => {}),
    /outside/,
  );
  for (const interval of ['nan', 'Infinity', '0', '-1', '86401'])
    await assert.rejects(cli(f, state, '--interval', interval));
});

test('importing observer does not load a profile or start a loop', async (t) => {
  const f = await fixture(t),
    missing = join(f.root, 'absent');
  const { stdout } = await run(
    process.execPath,
    [
      '--input-type=module',
      '-e',
      `await import(${JSON.stringify(new URL('../../examples/watch-issues.mjs', import.meta.url).href)})`,
    ],
    { cwd: f.root, env: { ...process.env, RAD_HOME: missing }, timeout: 30000 },
  );
  assert.equal(stdout, '');
  await assert.rejects(access(missing));
});
