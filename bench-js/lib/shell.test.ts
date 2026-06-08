import assert from 'node:assert/strict';
// bench-js/lib/shell.test.ts
import { test } from 'node:test';

import { parseHyperfine, parseSize, parseMemory } from './shell.ts';

test('parseHyperfine maps results to cli entries', () => {
  const json = { results: [{ command: 'rust nce', mean: 0.012, stddev: 0.001 }] };
  assert.deepEqual(parseHyperfine(json, '7 deps'), [
    { fixture: '7 deps', variant: 'rust nce', mean_s: 0.012, stddev_s: 0.001 },
  ]);
});

test('parseSize reads "name<TAB>bytes" lines', () => {
  assert.deepEqual(parseSize('riri-nce\t7340032\nriri-npd\t7000000\n'), [
    { binary: 'riri-nce', bytes: 7340032 },
    { binary: 'riri-npd', bytes: 7000000 },
  ]);
});

test('parseMemory reads peak/total kb lines', () => {
  assert.deepEqual(parseMemory('peak_kb=715\ntotal_kb=1050\n'), { peak_kb: 715, total_kb: 1050 });
});
