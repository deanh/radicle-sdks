import test from 'node:test';
import assert from 'node:assert/strict';
import { parseRepoId, parseDid, parseNodeId, parseOid, parseObjectId } from 'radicle-sdk';
import { fixture } from './helpers.mjs';

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
