import assert from 'node:assert/strict';
import { test } from 'node:test';
import { readFile, writeFile, rename, mkdir, rm, readdir } from 'node:fs/promises';
import { join } from 'node:path';
import { Issues } from '../dist/index.js';
import { fixture, git } from './helpers.mjs';
import { since, emitCursor, parseCursor, pollOnce } from '../../examples/reconcile-refs.mjs';

async function cursor(repo) {
  return emitCursor(
    repo.rid,
    new Map((await repo.namespaceTips()).map((t) => [t.nodeId, t.target])),
  );
}

test('reconciliation: offline operations collapse, idle is ref-only, forced COB is merely changed', async (t) => {
  const { repo, profile, signer, storage, rid } = await fixture(t);
  const issue = await Issues.openAs(repo, signer).create({ title: 'one', description: 'root' });
  const start = await cursor(repo);
  const idle = await since(repo, start);
  assert.deepEqual(idle.stats, { namespace_tips: 1, signed_refs_at: 0, commit: 0, ref_names: 0 });
  assert.deepEqual(idle.page.cobs, []);
  const scanned = await since(repo, start, { fullScan: true });
  assert.deepEqual(scanned.page, idle.page);
  assert.ok(scanned.stats.signed_refs_at > 0);
  await issue.comment('one');
  await issue.comment('two');
  const { page } = await since(repo, start);
  assert.equal(page.cobs.length, 1);
  assert.equal(page.cobs[0].kind, 'changed');
  const stored = join(storage.path, rid.slice(4));
  git(
    stored,
    'update-ref',
    `refs/namespaces/${profile.nodeId}/refs/cobs/xyz.radicle.issue/${issue.id}`,
    issue.id,
  );
  await repo.signRefs(signer);
  const rewind = await since(repo, page.next);
  assert.equal(rewind.page.cobs[0].kind, 'changed');
  assert.equal(rewind.page.namespaces[0].kind, 'updated');
  const sigref = `refs/namespaces/${profile.nodeId}/refs/rad/sigrefs`;
  git(stored, 'update-ref', sigref, parseCursor(start, rid).get(profile.nodeId));
  const forced = await since(repo, page.next);
  assert.equal(forced.page.namespaces[0].kind, 'forced');
});

test('reconciliation: gaps are explicit, bad current sets fail, awaited callbacks acknowledge only on success', async (t) => {
  const { repo, profile, signer, root, storage, rid } = await fixture(t);
  const state = join(root, 'cursor');
  assert.equal(await pollOnce(profile, rid, state, () => assert.fail('baseline')), null);
  const before = await readFile(state);
  const issue = await Issues.openAs(repo, signer).create({ title: 'created', description: 'root' });
  await assert.rejects(
    pollOnce(profile, rid, state, async () => {
      await Promise.resolve();
      throw Error('handler');
    }),
    /handler/,
  );
  assert.deepEqual(await readFile(state), before);
  let delivered;
  await pollOnce(profile, rid, state, async (page) => {
    await Promise.resolve();
    delivered = page;
  });
  assert.equal(await readFile(state, 'utf8'), delivered.next);
  const tips = parseCursor(delivered.next, rid);
  tips.set(profile.nodeId, '1'.repeat(40));
  const gap = await since(repo, emitCursor(rid, tips));
  assert.deepEqual(gap.page.incomplete, [profile.nodeId]);
  assert.equal(gap.page.namespaces[0].kind, 'history-unavailable');
  assert.ok(gap.page.cobs.every((record) => record.old_complete === false));
  assert.ok(gap.page.cobs.some((record) => record.id === issue.id));
  const stored = join(storage.path, rid.slice(4));
  await writeFile(
    join(stored, `refs/namespaces/${profile.nodeId}/refs/rad/sigrefs`),
    `${'2'.repeat(40)}\n`,
  );
  const saved = await readFile(state);
  await assert.rejects(
    pollOnce(profile, rid, state, () => assert.fail('read error')),
    (e) => e.code === 'GIT',
  );
  assert.deepEqual(await readFile(state), saved);
});

test('reconciliation checkpoint replace failure retains recoverable state and repeats delivery', async (t) => {
  const { profile, repo, signer, root, rid } = await fixture(t);
  const state = join(root, 'cursor');
  await pollOnce(profile, rid, state, () => assert.fail('baseline'));
  const old = await readFile(state);
  await Issues.openAs(repo, signer).create({ title: 'new', description: 'root' });
  const moved = `${state}.saved`;
  let delivered;
  await assert.rejects(
    pollOnce(profile, rid, state, async (page) => {
      delivered = page;
      await rename(state, moved);
      await mkdir(state);
    }),
  );
  assert.deepEqual(await readFile(moved), old);
  assert.ok(!(await readdir(root)).some((name) => name.startsWith('.cursor.')));
  await rm(state, { recursive: true });
  await rename(moved, state);
  const replayed = await pollOnce(profile, rid, state, (page) => assert.deepEqual(page, delivered));
  assert.deepEqual(replayed, delivered);
});

test('reconciliation sorts COB tuples and treats invalid advertised IDs as storage errors', async (t) => {
  const { profile, repo, signer, storage, rid } = await fixture(t);
  const stored = join(storage.path, rid.slice(4));
  const old = await cursor(repo);
  const base = git(stored, 'rev-parse', 'HEAD');
  for (const type of ['a.b', 'a', 'a-b']) {
    git(stored, 'update-ref', `refs/namespaces/${profile.nodeId}/refs/cobs/${type}/${base}`, base);
  }
  await repo.signRefs(signer);
  const { page } = await since(repo, old);
  assert.deepEqual(
    page.cobs.map((record) => record.typename),
    ['a', 'a-b', 'a.b'],
  );
  git(stored, 'update-ref', `refs/namespaces/${profile.nodeId}/refs/cobs/a/bad-id`, base);
  await repo.signRefs(signer);
  await assert.rejects(
    since(repo, old),
    (error) => error.code === 'GIT' && error.message.includes('invalid signed COB ref'),
  );
});

test('reconciliation cursor: canonical bytes and strict grammar', async (t) => {
  const { repo, profile } = await fixture(t);
  for (let i = 1; i <= 32; i++) {
    const tips = new Map([[profile.nodeId, i.toString(16).padStart(40, '0')]]);
    const text = emitCursor(repo.rid, tips);
    assert.deepEqual(parseCursor(text, repo.rid), tips);
    assert.equal(emitCursor(repo.rid, parseCursor(text, repo.rid)), text);
  }
  const text = await cursor(repo);
  for (const bad of [
    text.slice(0, -1),
    text + '\n',
    '\ufeff' + text,
    text.replaceAll('\n', '\r\n'),
    text.replace('v1', 'v2'),
    text.replace('/sigrefs', '/sigrefs '),
    text.replace(profile.nodeId, 'bad'),
  ]) {
    assert.throws(() => parseCursor(bad, repo.rid), /cursor line/);
  }
});
