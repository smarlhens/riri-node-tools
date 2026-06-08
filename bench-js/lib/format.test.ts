// bench-js/lib/format.test.ts
import assert from 'node:assert/strict';
import { test } from 'node:test';

import { formatNanoseconds, formatBytes } from './format.ts';

test('formatNanoseconds scales ns/µs/ms/s', () => {
  assert.equal(formatNanoseconds(500), '500.0 ns');
  assert.equal(formatNanoseconds(1120), '1.1 µs');
  assert.equal(formatNanoseconds(253390), '253.4 µs');
  assert.equal(formatNanoseconds(2437700), '2.44 ms');
  assert.equal(formatNanoseconds(2_500_000_000), '2.50 s');
});

test('formatBytes via pretty-bytes', () => {
  assert.equal(formatBytes(7340032), '7.34 MB');
  assert.equal(formatBytes(732160), '732 kB');
});
