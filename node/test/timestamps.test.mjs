import test from 'node:test';
import assert from 'node:assert/strict';
import { probeTimestamp } from '../dist/probes.js';
import { RadicleError } from '../dist/index.js';

test('worker timestamp conversion: Date boundaries, multiplication overflow, u64 maximum', async () => {
  for (const [value, seconds, want] of [
    ['0', false, 0], ['1700000000', true, 1700000000000], ['-1', true, -1000],
    ['8640000000000000', false, 8640000000000000], ['-8640000000000000', false, -8640000000000000],
    ['8640000000000', true, 8640000000000000], ['-8640000000000', true, -8640000000000000],
  ]) {
    const result = await probeTimestamp(value, seconds);
    assert.equal(result, want);
    assert.equal(typeof result, 'number');
    assert.equal(new Date(result).getTime(), result);
    assert.ok(Number.isSafeInteger(result));
  }
  for (const [value, seconds] of [
    ['8640000000000001', false], ['-8640000000000001', false],
    ['8640000000001', true], ['-8640000000001', true],
    ['9223372036854775807', true], ['-9223372036854775808', true],
  ]) {
    await assert.rejects(probeTimestamp(value, seconds), e => e instanceof RadicleError && e.code === 'INVALID_DATA');
  }
  // The probe accepts decimal i64 upstream values; overflow at its input boundary
  // exercises RangeError translation, not a public timestamp-input SDK feature.
  await assert.rejects(probeTimestamp('18446744073709551615', false), e => e instanceof RangeError && e.code === 'ERR_OUT_OF_RANGE');
  await assert.rejects(probeTimestamp('bad', true), e => e instanceof TypeError && e.code === 'ERR_INVALID_ARG_VALUE');
});
