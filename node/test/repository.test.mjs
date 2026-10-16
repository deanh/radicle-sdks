import test from 'node:test';
import assert from 'node:assert/strict';
import { join } from 'node:path';
import { mkdir, writeFile, rename, symlink, chmod } from 'node:fs/promises';
import { execFileSync } from 'node:child_process';
import { Profile, Issues, Patches, RadicleError } from '../dist/index.js';
import { fixture, git } from './helpers.mjs';
const stored = (f) => join(f.home, 'storage', f.rid.slice(4));
const ns = (f) => `refs/namespaces/${f.profile.nodeId}/refs/heads/main`;
const category = (code) => (e) => e instanceof RadicleError && e.code === code;
const invalid = (e) => e instanceof TypeError && e.code === 'ERR_INVALID_ARG_VALUE';
const wrongType = (e) => e instanceof TypeError && e.code === 'ERR_INVALID_ARG_TYPE';
function importCommits(f) {
  git(f.work, 'push', stored(f), `HEAD:${ns(f)}`);
  return git(f.work, 'rev-parse', 'HEAD');
}
function hashObject(f, type, input) {
  return execFileSync('git', ['-C', stored(f), 'hash-object', '-t', type, '-w', '--stdin'], {
    input,
    encoding: 'utf8',
  }).trim();
}

test('storage discovery preserves upstream order and errors; repository handles remain lazy', async (t) => {
  const f = await fixture(t);
  assert.equal(await f.storage.contains(f.rid), true);
  const list = await f.storage.repositories();
  assert.deepEqual(list, [f.rid]);
  assert.ok(Object.isFrozen(list));
  const other = (await Profile.init(join(f.root, 'empty'), 'empty')).storage;
  assert.deepEqual(await other.repositories(), []);
  assert.equal(await other.contains(f.rid), false);
  const missing = other.repository(f.rid);
  await assert.rejects(missing.branches(), category('REPOSITORY'));
  await assert.rejects(f.storage.contains('invalid'), invalid);
  await assert.rejects(f.storage.contains(42), wrongType);
  const path = join(f.home, 'storage');
  await rename(path, `${path}-moved`);
  try {
    await assert.rejects(f.storage.repositories(), category('STORAGE'));
  } finally {
    await rename(`${path}-moved`, path);
  }
});

test('branches are sorted canonical short names, not actor namespaced branches; only NotFound is null', async (t) => {
  const f = await fixture(t),
    base = (await f.repo.head()).oid;
  for (const name of ['z', 'a']) git(stored(f), 'update-ref', `refs/heads/${name}`, base);
  git(stored(f), 'update-ref', `refs/namespaces/${f.profile.nodeId}/refs/heads/private`, base);
  assert.deepEqual(await f.repo.branches(), ['a', 'main', 'z']);
  assert.ok(Object.isFrozen(await f.repo.branches()));
  assert.equal(await f.repo.branchHead('main'), base);
  assert.equal(await f.repo.branchHead('absent'), null);
  assert.equal(await f.repo.branchHead('refs/heads/main'), null); // treated as a short name, no normalization
  git(stored(f), 'symbolic-ref', 'refs/heads/alias', 'refs/heads/main');
  assert.equal(await f.repo.branchHead('alias'), base);
  for (const name of ['', 'bad..name', 'a\0b'])
    await assert.rejects(f.repo.branchHead(name), invalid);
  await assert.rejects(f.repo.branchHead(42), wrongType);
  await writeFile(join(stored(f), 'refs', 'heads', 'broken'), 'not-an-oid\n');
  await assert.rejects(f.repo.branchHead('broken'), category('GIT'));
  assert.ok(!(await f.repo.branches()).includes('broken')); // libgit2 skips malformed loose refs
  await writeFile(join(stored(f), 'packed-refs'), 'invalid packed references\n');
  await assert.rejects(f.repo.branches(), category('GIT'));
});

test('tree canonical ordering/modes and binary names, blob copies and full commit metadata', async (t) => {
  const f = await fixture(t),
    base = (await f.repo.head()).oid;
  await mkdir(join(f.work, 'a'));
  await writeFile(join(f.work, 'a', 'child'), 'nested\n');
  await writeFile(join(f.work, 'a.b'), 'before directory\n');
  await writeFile(join(f.work, 'executable'), 'exec\n');
  await chmod(join(f.work, 'executable'), 0o755);
  await symlink('README.md', join(f.work, 'link'));
  const binary = Buffer.from([0, 255, 128, 10]);
  await writeFile(Buffer.concat([Buffer.from(f.work + '/'), Buffer.from([255])]), binary);
  git(f.work, 'add', '.');
  git(f.work, 'update-index', '--add', '--cacheinfo', `160000,${base},submodule`);
  git(f.work, 'commit', '-m', 'tree fixture');
  const head = importCommits(f),
    commit = await f.repo.commit(head);
  assert.deepEqual(commit.parents, [base]);
  assert.ok(Object.isFrozen(commit.parents));
  const tree = await f.repo.tree(commit.tree);
  assert.ok(Object.isFrozen(tree));
  assert.ok(Object.isFrozen(tree.entries));
  const names = tree.entries.map((e) => e.nameStr);
  assert.ok(names.indexOf('a.b') < names.indexOf('a'));
  const entry = (name) => tree.entries.find((e) => e.nameStr === name);
  assert.equal(entry('a').kind, 'tree');
  assert.equal(entry('a').mode, 0o040000);
  assert.equal(entry('executable').mode, 0o100755);
  assert.equal(entry('link').kind, 'blob');
  assert.equal(entry('link').mode, 0o120000);
  assert.equal(entry('submodule').kind, 'commit');
  assert.equal(entry('submodule').mode, 0o160000);
  const raw = tree.entries.find((e) => e.name[0] === 255);
  assert.equal(raw.nameStr, '\ufffd');
  assert.ok(Object.isFrozen(raw));
  const blob = await f.repo.blob(raw.oid);
  assert.deepEqual(Buffer.from(blob), binary);
  blob[0] = 12;
  raw.name[0] = 1;
  assert.deepEqual(Buffer.from(await f.repo.blob(raw.oid)), binary);
  assert.equal((await f.repo.tree(commit.tree)).entries.at(-1).name[0], 255);
  assert.equal((await f.repo.tree(entry('a').oid)).entries[0].nameStr, 'child');
  assert.equal(Buffer.from(await f.repo.blob(entry('link').oid)).toString(), 'README.md');
  for (const [method, wrong] of [
    ['tree', head],
    ['blob', commit.tree],
    ['commit', raw.oid],
  ]) {
    await assert.rejects(f.repo[method](wrong), category('GIT'));
    await assert.rejects(f.repo[method]('0'.repeat(40)), category('GIT'));
    await assert.rejects(f.repo[method]('invalid'), invalid);
    await assert.rejects(f.repo[method](42), wrongType);
  }
  const id = hashObject(
    f,
    'commit',
    Buffer.concat([
      Buffer.from(`tree ${commit.tree}\nparent ${base}\nauthor A`),
      Buffer.from([255]),
      Buffer.from(' <a'),
      Buffer.from([255]),
      Buffer.from('@example.invalid> 100 +0130\ncommitter C'),
      Buffer.from([254]),
      Buffer.from(' <c@example.invalid> 200 -0230\nencoding ISO-8859-1\n\nmessage '),
      Buffer.from([255, 10]),
    ]),
  );
  const c = await f.repo.commit(id);
  assert.equal(c.authorNameStr, 'A\ufffd');
  assert.equal(c.authorEmailStr, 'a\ufffd@example.invalid');
  assert.equal(c.committerNameStr, 'C\ufffd');
  assert.equal(c.committerEmailStr, 'c@example.invalid');
  assert.equal(c.authorOffsetMin, 90);
  assert.equal(c.committerOffsetMin, -150);
  assert.equal(c.authorTime, 100000);
  assert.equal(c.committerTime, 200000);
  assert.equal(c.encoding, 'ISO-8859-1');
  assert.equal(c.messageStr, 'message \ufffd\n');
  for (const key of ['authorName', 'authorEmail', 'committerName', 'committerEmail', 'message']) {
    assert.ok(c[key] instanceof Uint8Array);
    const original = c[key][0];
    c[key][0] ^= 255;
    assert.equal((await f.repo.commit(id))[key][0], original);
  }
});

test('bounded history pages preserve revwalk order, validate JS numbers without coercion and resolve revision expressions', async (t) => {
  const f = await fixture(t),
    base = (await f.repo.head()).oid,
    chronological = [base];
  for (let i = 0; i < 4; i++) {
    await writeFile(join(f.work, 'README.md'), `commit ${i}\n`);
    git(f.work, 'commit', '-am', `commit ${i}`);
    chronological.push(git(f.work, 'rev-parse', 'HEAD'));
  }
  const head = importCommits(f),
    expected = chronological.toReversed();
  const page = await f.repo.history(ns(f), { limit: 2 });
  assert.equal(page.name, ns(f));
  assert.equal(page.head, head);
  assert.deepEqual(page.oids, expected.slice(0, 2));
  assert.equal(page.hasMore, true);
  assert.equal(page.offset, 0);
  assert.ok(Object.isFrozen(page));
  assert.ok(Object.isFrozen(page.oids));
  const middle = await f.repo.history(head, { limit: 2, offset: 2 });
  assert.deepEqual(middle.oids, expected.slice(2, 4));
  assert.equal(middle.hasMore, true);
  const last = await f.repo.history(head, { limit: 2, offset: 4 });
  assert.deepEqual(last.oids, [base]);
  assert.equal(last.hasMore, false);
  assert.deepEqual((await f.repo.history(`${ns(f)}~2`)).oids, expected.slice(2));
  assert.deepEqual((await f.repo.history(head)).oids, expected);
  assert.deepEqual((await f.repo.history(head, { limit: 1001 })).oids, expected);
  const exhausted = await f.repo.history(head, { offset: Number.MAX_SAFE_INTEGER });
  assert.deepEqual(exhausted.oids, []);
  assert.equal(exhausted.offset, Number.MAX_SAFE_INTEGER);
  assert.equal(exhausted.hasMore, false);
  for (const options of [
    { limit: 0 },
    { limit: -1 },
    { offset: -1 },
    { limit: 1.5 },
    { offset: 0.5 },
    { limit: NaN },
    { offset: Infinity },
    { limit: Number.MAX_SAFE_INTEGER + 1 },
  ])
    await assert.rejects(f.repo.history(head, options), invalid);
  for (const options of [null, { limit: '2' }, { offset: true }])
    await assert.rejects(f.repo.history(head, options), wrongType);
  await assert.rejects(f.repo.history('missing-ref'), category('GIT'));
  const c = await f.repo.commit(head);
  await assert.rejects(f.repo.history(c.tree), category('GIT'));
  // A merge must retain both parents; do not add a custom history sorting policy.
  const side = hashObject(
    f,
    'commit',
    Buffer.from(
      `tree ${c.tree}\nparent ${base}\nauthor A <a@x> 1 +0000\ncommitter A <a@x> 1 +0000\n\nside\n`,
    ),
  );
  const merge = hashObject(
    f,
    'commit',
    Buffer.from(
      `tree ${c.tree}\nparent ${head}\nparent ${side}\nauthor A <a@x> 2 +0000\ncommitter A <a@x> 2 +0000\n\nmerge\n`,
    ),
  );
  assert.deepEqual((await f.repo.commit(merge)).parents, [head, side]);
  const dag = (await f.repo.history(merge)).oids;
  assert.equal(dag[0], merge);
  assert.equal(new Set(dag).size, 7);
  assert.ok(dag.includes(side));
  const broken = hashObject(
    f,
    'commit',
    Buffer.from(
      `tree ${c.tree}\nparent ${'f'.repeat(40)}\nauthor A <a@x> 1 +0000\ncommitter A <a@x> 1 +0000\n\nbroken ancestry\n`,
    ),
  );
  for (const offset of [0, 1, 10])
    await assert.rejects(f.repo.history(broken, { offset }), category('GIT'));
});

test('history limits above 1000 really clamp while offset and hasMore remain accurate', async (t) => {
  const f = await fixture(t),
    base = (await f.repo.head()).oid;
  const stream =
    Array.from(
      { length: 1001 },
      (_, i) =>
        `commit refs/heads/pagination\nmark :${i + 1}\ncommitter A <a@x> 1 +0000\ndata 1\nx\nfrom ${i ? ':' + i : base}\n\n`,
    ).join('') + 'done\n';
  execFileSync('git', ['-C', stored(f), 'fast-import', '--quiet'], { input: stream });
  for (const limit of [1001, Number.MAX_SAFE_INTEGER]) {
    const page = await f.repo.history('refs/heads/pagination', { limit });
    assert.equal(page.oids.length, 1000);
    assert.equal(page.hasMore, true);
  }
  const last = await f.repo.history('refs/heads/pagination', { offset: 1000 });
  assert.equal(last.oids.length, 2);
  assert.equal(last.hasMore, false);
});

test('signRefs snapshots upstream results and shares the COB write lock across independent signer handles', async (t) => {
  const f = await fixture(t),
    base = (await f.repo.head()).oid;
  const issue = await Issues.openAs(f.repo, f.signer).create({
    title: 'Issue',
    description: 'Root',
  });
  const patch = await Patches.openAs(f.repo, f.signer).create({
    title: 'Patch',
    description: '',
    base,
    head: base,
  });
  const before = await f.repo.signRefs(f.signer);
  await writeFile(join(f.work, 'README.md'), 'import then sign\n');
  git(f.work, 'commit', '-am', 'import');
  const head = importCommits(f);
  const signed = await f.repo.signRefs(f.profile.signer());
  assert.equal(signed.nodeId, f.profile.nodeId);
  assert.equal(signed.refs['refs/heads/main'], head);
  assert.notEqual(signed.at, before.at);
  assert.equal(signed.parent, before.at);
  assert.ok(['none', 'root', 'parent'].includes(signed.featureLevel));
  assert.ok(Object.isFrozen(signed));
  assert.ok(Object.isFrozen(signed.refs));
  assert.notEqual(before.refs['refs/heads/main'], head);
  const results = await Promise.all([
    f.repo.signRefs(f.profile.signer()),
    issue.comment('Mixed write'),
    patch.label(['mixed']),
    f.repo.signRefs(f.profile.signer()),
  ]);
  const persisted = (at) =>
    Object.fromEntries(
      git(stored(f), 'show', `${at}:refs`)
        .split('\n')
        .map((line) => {
          const [oid, name] = line.split(' ');
          return [name, oid];
        })
        .filter(([name]) => name !== 'refs/rad/sigrefs-parent'),
    ); // upstream strips this structural entry
  for (const result of [signed, results[0], results[3]])
    assert.deepEqual(result.refs, persisted(result.at));
  const final = await f.repo.signRefs(f.signer);
  assert.equal(final.refs[`refs/cobs/xyz.radicle.issue/${issue.id}`], results[1].entryId);
  assert.equal(final.refs[`refs/cobs/xyz.radicle.patch/${patch.id}`], results[2].entryId);
  assert.equal(
    git(stored(f), 'rev-parse', `refs/namespaces/${f.profile.nodeId}/refs/rad/sigrefs`),
    final.at,
  );
  assert.equal((await f.repo.head()).oid, base); // signing does not move canonical Git head
  await assert.rejects(f.repo.signRefs({}), wrongType);
  assert.equal((await f.repo.signRefs(f.signer)).refs['refs/heads/main'], head);
});
