import test from 'node:test';
import assert from 'node:assert/strict';
import {
  parseRepoId,
  parseDid,
  parseNodeId,
  parseOid,
  parseObjectId,
  Home,
  Storage,
  Profile,
  Issues,
  Patches,
  RadicleError,
} from 'radicle-sdk';
import { join } from 'node:path';
import { access, mkdir, writeFile, realpath } from 'node:fs/promises';
import { execFileSync } from 'node:child_process';
import { fixture } from './helpers.mjs';

const category = (code) => (e) => e instanceof RadicleError && e.code === code;
const invalid = (e) => e instanceof TypeError && e.code === 'ERR_INVALID_ARG_VALUE';
const wrongType = (e) => e instanceof TypeError && e.code === 'ERR_INVALID_ARG_TYPE';

test('home factories, paths, explicit storage and aliases use public async handles', async (t) => {
  const f = await fixture(t);
  assert.equal(f.profile.home.path, await realpath(f.home));
  assert.equal(f.storage.path, join(await realpath(f.home), 'storage'));
  assert.equal(f.signer.did, f.profile.did);
  const missing = join(f.root, 'not-created');
  await assert.rejects(Home.load(missing), category('STORAGE'));
  await assert.rejects(access(missing));
  const incomplete = join(f.root, 'incomplete');
  await mkdir(incomplete);
  await assert.rejects(Home.load(incomplete), category('STORAGE'));
  const created = await Home.create(missing);
  assert.ok(created instanceof Home);
  assert.equal((await Home.load(created.path)).path, created.path);
  assert.equal((await Home.create(missing)).path, created.path);
  const resolved = execFileSync(
    process.execPath,
    [
      '--input-type=module',
      '-e',
      "import {Home} from 'radicle-sdk'; console.log((await Home.resolve()).path)",
    ],
    { cwd: process.cwd(), env: { ...process.env, RAD_HOME: created.path }, encoding: 'utf8' },
  ).trim();
  assert.equal(resolved, created.path);
  const blocker = join(f.root, 'file');
  await writeFile(blocker, 'not a directory');
  await assert.rejects(Home.create(blocker), category('STORAGE'));
  await assert.rejects(Home.load(42), wrongType);
  await assert.rejects(Home.create(null), wrongType);
  const other = await Storage.open(join(f.root, 'explicit'), 'explicit', f.profile.nodeId);
  assert.ok(other instanceof Storage);
  assert.equal(other.path, join(f.root, 'explicit'));
  assert.deepEqual(await other.repositories(), []);
  const reopened = await Storage.open(f.storage.path, 'alice', f.profile.nodeId);
  assert.equal(await reopened.contains(f.rid), true);
  const invalidPath = join(f.root, 'invalid');
  await assert.rejects(Storage.open(invalidPath, 'bad alias!', f.profile.nodeId), invalid);
  await assert.rejects(Storage.open(invalidPath, 'alice', 'bad-node'), invalid);
  await assert.rejects(access(invalidPath));
  await assert.rejects(Storage.open(42, 'alice', f.profile.nodeId), wrongType);
  // Upstream accepts an existing path even if it is a file; discovery fails on use.
  const blocked = await Storage.open(blocker, 'alice', f.profile.nodeId);
  await assert.rejects(blocked.repositories(), category('STORAGE'));
  const aliases = f.profile.aliases;
  assert.equal(await aliases.get(f.profile.nodeId), 'alice');
  const grouped = await aliases.reverseLookup('alice');
  assert.deepEqual(grouped, { alice: [f.profile.nodeId] });
  assert.ok(Object.isFrozen(grouped));
  assert.ok(Object.isFrozen(grouped.alice));
  assert.deepEqual(await aliases.reverseLookup('unknown'), {});
  const peer = await Profile.init(join(f.root, 'peer'), 'peer');
  assert.equal(await aliases.get(peer.nodeId), null);
  await assert.rejects(aliases.get('invalid'), invalid);
  await assert.rejects(aliases.reverseLookup('bad alias!'), invalid);
  await assert.rejects(aliases.get(42), wrongType);
  await assert.rejects(aliases.reverseLookup(null), wrongType);
  const hostile = await Profile.init(join(f.root, 'hostile'), '__proto__');
  const own = await hostile.aliases.reverseLookup('__proto__');
  assert.ok(Object.hasOwn(own, '__proto__'));
  assert.deepEqual(own.__proto__, [hostile.nodeId]);
});

test('typed store metadata, counts and snapshot conveniences track writes without cached state', async (t) => {
  const f = await fixture(t);
  const issues = Issues.open(f.repo),
    patches = Patches.open(f.repo);
  for (const store of [issues, patches]) {
    assert.equal(store.rid, f.rid);
    assert.equal(await store.count(), 0);
    assert.equal(await store.isEmpty(), true);
  }
  const writer = Issues.openAs(f.repo, f.signer);
  const issue = await writer.create({ title: 't', description: 'root' });
  const root = (await issue.reload()).commentIds[0];
  const result = await issue.comment('reply');
  assert.equal(result.snapshot.commentCount, 2);
  assert.deepEqual(result.snapshot.commentIds, [root, result.entryId]);
  assert.deepEqual(
    result.snapshot.commentIds,
    result.snapshot.comments.map((c) => c.id),
  );
  assert.ok(Object.isFrozen(result.snapshot.commentIds));
  assert.equal(await issues.count(), 1);
  assert.equal(await issues.isEmpty(), false);
  const base = (await f.repo.head()).oid;
  const patch = await Patches.openAs(f.repo, f.signer).create({
    title: 'p',
    description: 'd',
    base,
    head: base,
  });
  const next = await patch.update({ description: 'next', base, head: base });
  assert.equal(next.snapshot.revisionCount, 2);
  assert.equal(next.snapshot.revisions.length, 2);
  assert.equal(await patches.count(), 1);
  assert.equal(await patches.isEmpty(), false);
  const missing = (await Profile.init(join(f.root, 'empty'), 'empty')).storage.repository(f.rid);
  for (const store of [Issues.open(missing), Patches.open(missing)]) {
    assert.equal(store.rid, f.rid);
    await assert.rejects(store.count(), category('REPOSITORY'));
    await assert.rejects(store.isEmpty(), category('REPOSITORY'));
  }
});

test('public ID parsers canonicalize using upstream domains and reject without coercion', async (t) => {
  const f = await fixture(t);
  const oid = (await f.repo.head()).oid;
  for (const [parse, value, expected] of [
    [parseRepoId, f.rid, f.rid],
    [parseDid, f.profile.did, f.profile.did],
    [parseNodeId, f.profile.nodeId, f.profile.nodeId],
    [parseOid, oid.toUpperCase(), oid],
    [parseObjectId, oid.toUpperCase(), oid],
  ]) {
    assert.equal(parse(value), expected);
    for (const bad of ['', 'invalid', 'a\0b']) {
      assert.throws(
        () => parse(bad),
        (e) => e instanceof TypeError && e.code === 'ERR_INVALID_ARG_VALUE',
      );
    }
    for (const bad of [null, undefined, 42, {}, ['id']]) {
      assert.throws(
        () => parse(bad),
        (e) => e instanceof TypeError && e.code === 'ERR_INVALID_ARG_TYPE',
      );
    }
  }
});
