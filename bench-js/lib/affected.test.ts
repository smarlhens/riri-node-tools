import assert from 'node:assert/strict';
// bench-js/lib/affected.test.ts
import { test } from 'node:test';

import { affectedBenchableCrates, type CargoMetadata } from './affected.ts';

// Mirror of the real workspace dependency graph (path-deps only).
const DEPS: Record<string, string[]> = {
  'riri-nce': ['riri-common', 'riri-node-lifecycle', 'riri-npm', 'riri-pnpm', 'riri-semver-range', 'riri-yarn'],
  'riri-ncd': ['riri-common', 'riri-npm', 'riri-pnpm', 'riri-semver-range', 'riri-workspace', 'riri-yarn'],
  'riri-npd': ['riri-common', 'riri-npm', 'riri-pnpm', 'riri-workspace', 'riri-yarn'],
  'riri-semver-range': [],
  'riri-common': ['riri-find-up'],
  'riri-pnpm': ['riri-find-up'],
  'riri-workspace': ['riri-find-up'],
  'riri-find-up': [],
  'riri-npm': [],
  'riri-yarn': [],
  'riri-node-lifecycle': [],
  'riri-napi-nce': ['riri-nce', 'riri-semver-range'],
  xtask: ['riri-node-lifecycle'],
};

const META: CargoMetadata = {
  packages: Object.entries(DEPS).map(([name, deps]) => ({
    name,
    manifest_path: `/repo/crates/${name}/Cargo.toml`,
    dependencies: deps.map(dep => ({ name: dep })),
  })),
};

const affected = (...files: string[]): string[] => affectedBenchableCrates(files, META);

test('data change in node-lifecycle affects only nce', () => {
  assert.deepEqual(affected('crates/riri-node-lifecycle/data/node-versions.json'), ['nce']);
});

test('semver-range affects its dependents nce and ncd, not npd', () => {
  assert.deepEqual(affected('crates/riri-semver-range/src/lib.rs'), ['nce', 'ncd', 'semver-range']);
});

test('workspace affects ncd and npd, not nce', () => {
  assert.deepEqual(affected('crates/riri-workspace/src/lib.rs'), ['ncd', 'npd']);
});

test('common (transitively used everywhere) affects all binary crates', () => {
  assert.deepEqual(affected('crates/riri-common/src/lib.rs'), ['nce', 'ncd', 'npd']);
});

test('find-up reaches every crate through common/pnpm/workspace', () => {
  assert.deepEqual(affected('crates/riri-find-up/src/lib.rs'), ['nce', 'ncd', 'npd']);
});

test('a single binary crate affects only itself', () => {
  assert.deepEqual(affected('crates/riri-npd/src/lib.rs'), ['npd']);
});

test('crates with no benchable dependents affect nothing', () => {
  assert.deepEqual(affected('crates/xtask/src/main.rs'), []);
  assert.deepEqual(affected('crates/riri-napi-nce/src/lib.rs'), []);
});

test('multiple changed crates union their affected sets', () => {
  assert.deepEqual(affected('crates/riri-npd/src/lib.rs', 'crates/riri-node-lifecycle/src/data.rs'), ['nce', 'npd']);
});

test('non-crate paths force all benchable crates', () => {
  const all = ['nce', 'ncd', 'npd', 'semver-range'];
  assert.deepEqual(affected('Cargo.lock'), all);
  assert.deepEqual(affected('fixtures/npm-v3-500-deps/package.json'), all);
  assert.deepEqual(affected('.github/workflows/bench-run.yml'), all);
  assert.deepEqual(affected('rustfmt.toml'), all);
});

test('an unknown crate path is treated conservatively as force-all', () => {
  assert.deepEqual(affected('crates/riri-brand-new/src/lib.rs'), ['nce', 'ncd', 'npd', 'semver-range']);
});
