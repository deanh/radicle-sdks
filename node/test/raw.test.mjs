import test from 'node:test';
import assert from 'node:assert/strict';
import { join } from 'node:path';
import { writeFile } from 'node:fs/promises';
import { execFileSync } from 'node:child_process';
import { Raw, Profile, Issues, Patches, RadicleError } from 'radicle-sdk';
import { fixture, git } from './helpers.mjs';
const ISSUE = 'xyz.radicle.issue',
  PATCH = 'xyz.radicle.patch';
const stored = (f) => join(f.home, 'storage', f.rid.slice(4));
const category = (code) => (e) => e instanceof RadicleError && e.code === code;
const invalid = (e) => e instanceof TypeError && e.code === 'ERR_INVALID_ARG_VALUE';
const wrongType = (e) => e instanceof TypeError && e.code === 'ERR_INVALID_ARG_TYPE';
const refs = (f) => git(stored(f), 'for-each-ref', '--format=%(refname) %(objectname)');

async function largeOp(f) {
  const issue = await Issues.openAs(f.repo, f.signer).create({
    title: 'json',
    description: 'x'.repeat(12000),
  });
  const raw = Raw.open(f.repo),
    op = (await raw.history(ISSUE, issue.id))[0];
  const index = op.actions.findIndex((a) => a.byteLength > 12000);
  assert.ok(index >= 0);
  return { raw, op, issue, index };
}
function inject(op, index, payload) {
  const bytes = typeof payload === 'string' ? new TextEncoder().encode(payload) : payload;
  op.actions[index].fill(32);
  op.actions[index].set(bytes);
  return () => op.actionsJson()[index];
}

test('raw facade is lazy, strict COB enumeration is sorted, and backend failures propagate', async (t) => {
  const f = await fixture(t),
    raw = Raw.open(f.repo);
  assert.equal(raw.rid, f.rid);
  assert.deepEqual(await raw.entities(ISSUE), []);
  assert.deepEqual(await raw.refs(ISSUE, '0'.repeat(40)), []);
  assert.deepEqual(await raw.history(ISSUE, '0'.repeat(40)), []);
  const issue = await Issues.openAs(f.repo, f.signer).create({ title: 't', description: 'd' });
  const base = (await f.repo.head()).oid,
    ns = `refs/namespaces/${f.profile.nodeId}/refs`;
  for (const name of [
    `refs/tags/xyz.fake.type/${base}`,
    `refs/heads/xyz.fake.branch/${base}`,
    `refs/cobs/${ISSUE}/not-an-id`,
    `${ns}/cobs/${ISSUE}/not-an-id`,
    `${ns}/cobs/bad_typename/${base}`,
  ])
    git(stored(f), 'update-ref', name, base);
  assert.deepEqual(await raw.typenames(), ['xyz.radicle.id', ISSUE]);
  assert.deepEqual(await raw.entities(ISSUE), [issue.id]);
  const result = await raw.refs(ISSUE, issue.id);
  assert.ok(Object.isFrozen(result));
  assert.ok(result.every(Object.isFrozen));
  assert.equal(result[0].target, issue.id);
  assert.ok(result[0].name.startsWith(`${ns}/cobs/`));
  for (const method of ['entities', 'refs', 'history']) {
    await assert.rejects(raw[method]('bad_typename', issue.id), invalid);
    await assert.rejects(raw[method](42, issue.id), wrongType);
  }
  for (const method of ['refs', 'history']) {
    await assert.rejects(raw[method](ISSUE, 'bad-id'), invalid);
    await assert.rejects(raw[method](ISSUE, null), wrongType);
  }
  const missing = Raw.open(
    (await Profile.init(join(f.root, 'empty'), 'empty')).storage.repository(f.rid),
  );
  assert.equal(missing.rid, f.rid);
  await assert.rejects(missing.typenames(), category('REPOSITORY'));
  await writeFile(join(stored(f), 'packed-refs'), 'invalid packed references\n');
  await assert.rejects(raw.typenames(), category('GIT'));
  await assert.rejects(raw.entities(ISSUE), category('GIT'));
});

test('causal history deduplicates across tips, orders independent branches and excludes identity/related links', async (t) => {
  const f = await fixture(t),
    raw = Raw.open(f.repo);
  const issue = await Issues.openAs(f.repo, f.signer).create({
    title: 'branch',
    description: 'root',
  });
  const aliceRef = `refs/namespaces/${f.profile.nodeId}/refs/cobs/${ISSUE}/${issue.id}`;
  const a = (await issue.comment('alice')).entryId;
  git(stored(f), 'update-ref', aliceRef, issue.id);
  const peer = await Profile.init(join(f.root, 'peer'), 'peer');
  const b = (await (await Issues.openAs(f.repo, peer.signer()).getMut(issue.id)).comment('bob'))
    .entryId;
  git(stored(f), 'update-ref', aliceRef, a);
  const before = refs(f),
    ops = await raw.history(ISSUE, issue.id);
  assert.deepEqual(
    ops.map((o) => o.id),
    [issue.id, ...[a, b].sort()],
  );
  assert.equal(new Set(ops.map((o) => o.id)).size, 3);
  for (let i = 0; i < ops.length; i++)
    for (const p of ops[i].parents) assert.ok(ops.findIndex((o) => o.id === p) < i);
  assert.equal(typeof ops[0].timestamp, 'bigint');
  assert.ok(ops[0].timestamp > BigInt(Math.floor(Date.now() / 1000) - 3600));
  assert.equal(ops[0].manifest.version, 1n);
  assert.equal(ops[0].manifest.typeName, ISSUE);
  assert.ok(Object.isFrozen(ops));
  assert.ok(ops.every(Object.isFrozen));
  assert.ok(Object.isFrozen(ops[0].parents));
  assert.ok(Object.isFrozen(ops[0].manifest));
  const identities = new Set(
    (await raw.history('xyz.radicle.id', (await raw.entities('xyz.radicle.id'))[0])).map(
      (o) => o.id,
    ),
  );
  assert.ok(ops[0].identity);
  assert.ok(ops.every((o) => !identities.has(o.id)));
  assert.equal(refs(f), before);
  const base = (await f.repo.head()).oid;
  const patch = await Patches.openAs(f.repo, f.signer).create({
    title: 'p',
    description: 'd',
    base,
    head: base,
  });
  const patchOps = await raw.history(PATCH, patch.id);
  assert.equal(patchOps.length, 1);
  assert.ok(patchOps[0].related.includes(base));
  assert.ok(!patchOps.some((o) => o.id === base));
  const after = refs(f);
  await raw.history(PATCH, patch.id);
  await raw.refs(PATCH, patch.id);
  assert.equal(refs(f), after);
});

test('raw action views are independent; decoder preserves integer tokens and frozen ordinary JSON values', async (t) => {
  const f = await fixture(t),
    { raw, op, issue, index } = await largeOp(f);
  const original = (await raw.history(ISSUE, issue.id))[0].actions[index].slice();
  const decode = (payload) => inject(op, index, payload)();
  for (const n of [
    '9007199254740992',
    '9007199254740993',
    '-9007199254740993',
    '18446744073709551615',
    '9'.repeat(3000),
  ]) {
    assert.equal(decode(n), BigInt(n));
  }
  for (const n of ['9007199254740991', '-9007199254740991', '42', '0'])
    assert.equal(decode(n), Number(n));
  assert.ok(Object.is(decode('-0'), -0));
  // Decimal/exponent tokens deliberately follow finite IEEE-754/Python floats.
  assert.equal(decode('1.25'), 1.25);
  assert.equal(decode('1e30'), 1e30);
  assert.equal(decode('9007199254740993e0'), Number('9007199254740993e0'));
  const nested = decode(
    '{"text":"9007199254740993","a":[9007199254740993,true,null],"__proto__":{"x":1}}',
  );
  assert.equal(nested.text, '9007199254740993');
  assert.equal(nested.a[0], 9007199254740993n);
  assert.ok(Object.isFrozen(nested));
  assert.ok(Object.isFrozen(nested.a));
  assert.ok(Object.hasOwn(nested, '__proto__'));
  assert.equal(nested.__proto__.x, 1);
  assert.equal({}.x, undefined);
  assert.equal(decode('"\\ud83d\\ude00"'), '😀');
  assert.deepEqual((await raw.history(ISSUE, issue.id))[0].actions[index], original);
  assert.ok(Object.isFrozen(op.actionsJson()));
});

test('JSON errors, overflow, invalid UTF-8 and unpaired escapes reject as COB without hiding raw bytes', async (t) => {
  const f = await fixture(t),
    { op, index } = await largeOp(f);
  for (const payload of [
    '{',
    '01',
    'NaN',
    '1e400',
    '"\\ud800"',
    '{"\\udc00":1}',
    '\ufeff{}',
    new Uint8Array([255]),
  ]) {
    assert.throws(inject(op, index, payload), category('COB'));
    assert.ok(op.actions[index] instanceof Uint8Array);
  }
});

test('bad entry tips reject whole history and raw timestamps keep exact upstream seconds', async (t) => {
  const f = await fixture(t),
    { raw, op, issue } = await largeOp(f);
  const ref = `refs/namespaces/${f.profile.nodeId}/refs/cobs/${ISSUE}/${issue.id}`;
  const original = git(stored(f), 'cat-file', 'commit', op.id);
  const altered = original.replace(
    /(committer .*?> )\d+ ([+-]\d{4})/,
    (_, prefix, zone) => `${prefix}9223372036854775807 ${zone}`,
  );
  const id = execFileSync(
    'git',
    ['-C', stored(f), 'hash-object', '--literally', '-t', 'commit', '-w', '--stdin'],
    { input: altered + '\n', encoding: 'utf8' },
  ).trim();
  // Raw loading parses signature metadata, not authentication. Copied signatures
  // do not authenticate these altered fixture commits; sigrefs are unchanged.
  git(stored(f), 'update-ref', ref, id);
  assert.equal((await raw.history(ISSUE, issue.id))[0].timestamp, 9223372036854775807n);
  git(stored(f), 'update-ref', ref, (await f.repo.head()).oid);
  await assert.rejects(raw.history(ISSUE, issue.id), category('COB'));
  git(stored(f), 'update-ref', ref, op.id);
  assert.equal((await raw.history(ISSUE, issue.id))[0].id, op.id);
});
