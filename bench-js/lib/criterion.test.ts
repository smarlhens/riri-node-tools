// bench-js/lib/criterion.test.ts
import assert from 'node:assert/strict';
import { resolve, dirname } from 'node:path';
import { test } from 'node:test';
import { fileURLToPath } from 'node:url';

import { readEstimates, crossCompare } from './criterion.ts';

const here = dirname(fileURLToPath(import.meta.url));

test('readEstimates rounds mean & stddev', () => {
  assert.deepEqual(readEstimates(resolve(here, '__fixtures__/estimates.json')), { meanNs: 14861, stdDevNs: 120 });
});

test('crossCompare pairs own vs reference on the "_ " suffix', () => {
  const out = crossCompare(
    [
      { name: 'riri_ satisfies', ns: 24 },
      { name: 'nodejs-semver_ satisfies', ns: 48 },
      { name: 'riri_ parse range', ns: 2000 },
    ],
    ['nodejs-semver'],
  );
  assert.equal(out.length, 1);
  assert.deepEqual(out[0], {
    metric: 'satisfies',
    ownName: 'riri',
    ownNs: 24,
    referenceName: 'nodejs-semver',
    referenceNs: 48,
    speedup: 2,
  });
});
