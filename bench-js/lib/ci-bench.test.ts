import assert from 'node:assert/strict';
// bench-js/lib/ci-bench.test.ts
import { test } from 'node:test';

import {
  benchTargets,
  packumentNames,
  parseMaxRssKb,
  parseTestCount,
  resolveCrateSet,
  type CargoMetadataTargets,
} from './ci-bench.ts';

const META: CargoMetadataTargets = {
  packages: [
    {
      name: 'riri-nce',
      targets: [
        { name: 'riri-nce', kind: ['lib'] },
        { name: 'nce', kind: ['bin'] },
        { name: 'check_engines', kind: ['bench'] },
      ],
    },
    {
      name: 'riri-semver-range',
      targets: [
        { name: 'riri-semver-range', kind: ['lib'] },
        { name: 'range_satisfies', kind: ['bench'] },
        { name: 'range_parsing', kind: ['bench'] },
      ],
    },
    { name: 'riri-npd', targets: [{ name: 'pin_dependencies', kind: ['bench'] }] },
  ],
};

test('resolveCrateSet defaults to all benchable crates', () => {
  assert.deepEqual(resolveCrateSet(''), {
    set: ['nce', 'ncd', 'npd', 'semver-range'],
    buildPackages: ['riri-nce', 'riri-ncd', 'riri-npd'],
  });
});

test('resolveCrateSet maps only binary crates to build packages', () => {
  assert.deepEqual(resolveCrateSet('nce semver-range'), {
    set: ['nce', 'semver-range'],
    buildPackages: ['riri-nce'],
  });
});

test('benchTargets returns only bench-kind targets for the set, sorted', () => {
  assert.deepEqual(benchTargets(['nce', 'semver-range'], META), ['check_engines', 'range_parsing', 'range_satisfies']);
});

test('benchTargets ignores crates outside the set', () => {
  assert.deepEqual(benchTargets(['npd'], META), ['pin_dependencies']);
});

test('parseMaxRssKb extracts the GNU time value', () => {
  assert.equal(parseMaxRssKb('\tMaximum resident set size (kbytes): 71540\n\tOther: 1'), 71540);
  assert.equal(parseMaxRssKb('no measurement here'), 0);
});

test('parseTestCount counts lines ending in ": test"', () => {
  const list = ['riri_nce::a: test', 'riri_nce::b: test', 'some other line', '0 benchmarks', ''].join('\n');
  assert.equal(parseTestCount(list), 2);
});

test('packumentNames strips the node_modules prefix', () => {
  const lockfile = JSON.stringify({
    packages: { '': {}, 'node_modules/foo': {}, 'node_modules/@scope/bar': {} },
  });
  assert.deepEqual(packumentNames(lockfile), ['foo', '@scope/bar']);
});
