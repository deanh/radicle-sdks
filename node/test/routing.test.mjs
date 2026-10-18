import test from 'node:test';
import assert from 'node:assert/strict';
import { join } from 'node:path';
import { access, rename, writeFile } from 'node:fs/promises';
import { DatabaseSync } from 'node:sqlite';
import { Profile, RoutingStore, RadicleError } from 'radicle-sdk';
import { fixture } from './helpers.mjs';
const category = (code) => (e) => e instanceof RadicleError && e.code === code;
const invalid = (e) => e instanceof TypeError && e.code === 'ERR_INVALID_ARG_VALUE';
const wrongType = (e) => e instanceof TypeError && e.code === 'ERR_INVALID_ARG_TYPE';

test('read-only routing exposes sorted inventories/entries, counts, and Date-safe timestamps', async (t) => {
  const f = await fixture(t),
    routes = await f.profile.routingStore();
  assert.equal(await routes.isEmpty(), true);
  assert.equal(await routes.total(), 0);
  assert.equal(await routes.count(f.rid), 0);
  assert.deepEqual(await routes.entries(), []);
  assert.deepEqual(await routes.get(f.rid), []);
  assert.deepEqual(await routes.getInventory(f.profile.nodeId), []);
  assert.equal(await routes.entry(f.rid, f.profile.nodeId), null);
  const peer = await Profile.init(join(f.root, 'peer'), 'peer');
  const db = new DatabaseSync(join(f.home, 'node', 'node.db'));
  const addNode = db.prepare(
    'INSERT OR IGNORE INTO nodes(id,features,alias,timestamp) VALUES (?,0,?,0)',
  );
  addNode.run(f.profile.nodeId, 'alice');
  addNode.run(peer.nodeId, 'peer');
  const insert = db.prepare('INSERT OR REPLACE INTO routing(repo,node,timestamp) VALUES (?,?,?)');
  try {
    insert.run(f.rid, f.profile.nodeId, 1234);
    insert.run(f.rid, peer.nodeId, 8640000000000000n);
    assert.deepEqual(await routes.get(f.rid), [peer.nodeId, f.profile.nodeId].sort());
    assert.deepEqual(await routes.getInventory(peer.nodeId), [f.rid]);
    assert.equal(await routes.count(f.rid), 2);
    assert.equal(await routes.total(), 2);
    assert.equal(await routes.isEmpty(), false);
    assert.equal(await routes.entry(f.rid, peer.nodeId), 8640000000000000);
    const entries = await routes.entries();
    assert.deepEqual(
      entries.map((e) => e.nodeId),
      [peer.nodeId, f.profile.nodeId].sort(),
    );
    assert.ok(Object.isFrozen(entries));
    assert.ok(entries.every(Object.isFrozen));
    assert.ok(Object.isFrozen(await routes.get(f.rid)));
    assert.ok(Object.isFrozen(await routes.getInventory(peer.nodeId)));
    for (const timestamp of [8640000000000001n, 9223372036854775807n]) {
      insert.run(f.rid, peer.nodeId, timestamp);
      assert.equal(await routes.entry(f.rid, peer.nodeId), null);
      assert.equal((await routes.entries()).find((e) => e.nodeId === peer.nodeId).timestamp, null);
      assert.equal(await routes.count(f.rid), 2); // bad metadata never hides the entry
    }
    insert.run(f.rid, peer.nodeId, -1n);
    await assert.rejects(routes.entry(f.rid, peer.nodeId), category('DATABASE'));
    await assert.rejects(routes.entries(), category('DATABASE'));
    insert.run(f.rid, peer.nodeId, 42);
    const reads = await Promise.all(Array.from({ length: 12 }, () => routes.get(f.rid)));
    assert.ok(reads.every((ids) => ids.length === 2));
    const independent = await RoutingStore.fromProfile(f.profile);
    assert.equal(await independent.total(), 2);
    insert.run(f.rid, peer.nodeId, 42);
    assert.equal(await routes.entry(f.rid, peer.nodeId), 42);
    assert.equal(await independent.entry(f.rid, peer.nodeId), 42);
    // Only the external writer changes content; native reads cannot mutate it.
    assert.equal(db.prepare('SELECT COUNT(*) AS n FROM routing').get().n, 2);
  } finally {
    db.close();
  }
});

test('routing factories reject missing/incompatible databases without creating or migrating them', async (t) => {
  const f = await fixture(t),
    path = join(f.home, 'node', 'node.db');
  await rename(path, `${path}.saved`);
  await assert.rejects(RoutingStore.fromProfile(f.profile), category('DATABASE'));
  await assert.rejects(access(path));
  await writeFile(path, '');
  // Upstream connection configuration rejects this empty file; no migrations run.
  await assert.rejects(RoutingStore.fromProfile(f.profile), category('DATABASE'));
  const db = new DatabaseSync(path);
  assert.deepEqual(db.prepare('SELECT name FROM sqlite_master').all(), []);
  db.close();
  await assert.rejects(RoutingStore.fromProfile({}), wrongType);
  await assert.rejects(RoutingStore.fromProfile(null), wrongType);
});

test('routing validates before queries and contains upstream unchecked row decoding panics', async (t) => {
  const f = await fixture(t),
    routes = await f.profile.routingStore();
  for (const method of ['get', 'count']) {
    await assert.rejects(routes[method]('bad-rid'), invalid);
    await assert.rejects(routes[method](42), wrongType);
  }
  await assert.rejects(routes.getInventory('bad-node'), invalid);
  await assert.rejects(routes.entry(f.rid, 'bad-node'), invalid);
  await assert.rejects(routes.entry('bad-rid', f.profile.nodeId), invalid);
  await assert.rejects(routes.entry(f.rid, null), wrongType);
  const db = new DatabaseSync(join(f.home, 'node', 'node.db'));
  try {
    db.exec('PRAGMA foreign_keys=OFF'); // intentional corrupt fixture
    db.prepare('INSERT INTO routing(repo,node,timestamp) VALUES (?,?,?)').run(f.rid, 'invalid', 1);
    // Pinned upstream routing::get uses unchecked Row::read. The shared worker
    // containment turns this panic into INTERNAL rather than unwinding into JS.
    await assert.rejects(routes.get(f.rid), category('INTERNAL'));
    db.exec('DELETE FROM routing');
    assert.deepEqual(await routes.get(f.rid), []);
    db.exec('DROP TABLE routing');
    await assert.rejects(routes.get(f.rid), category('DATABASE'));
    await assert.rejects(routes.total(), category('DATABASE'));
  } finally {
    db.close();
  }
});
