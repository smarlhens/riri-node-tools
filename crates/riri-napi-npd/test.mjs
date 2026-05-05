import { strict as assert } from 'node:assert';
import { readFileSync, readdirSync } from 'node:fs';
import { createRequire } from 'node:module';
import { resolve, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const require = createRequire(import.meta.url);
const __dirname = dirname(fileURLToPath(import.meta.url));

const nodeFile = readdirSync(__dirname).find(f => f.startsWith('npm-pin-dependencies.') && f.endsWith('.node'));
if (!nodeFile) {
  console.error('No .node binary found. Run `npx napi build --platform` first.');
  process.exit(1);
}
const napi = require(`./${nodeFile}`);

const fixturesDir = resolve(__dirname, '../../fixtures');

let passed = 0;
let failed = 0;

const test = (name, fn) => {
  try {
    fn();
    passed += 1;
    console.log(`  ✓ ${name}`);
  } catch (error) {
    failed += 1;
    console.error(`  ✗ ${name}`);
    console.error(error);
  }
};

const readFixture = name => {
  const dir = resolve(fixturesDir, name);
  return {
    pkg: readFileSync(resolve(dir, 'package.json'), 'utf8'),
    lock: readFileSync(resolve(dir, 'package-lock.json'), 'utf8'),
  };
};

test('pinDependencies returns pins for npm v3 unpinned deps', () => {
  const { pkg, lock } = readFixture('npd-npm-v3-unpinned-deps');
  const result = napi.pinDependencies({
    packageJson: pkg,
    lockfileContent: lock,
    lockfileType: 'npm',
  });
  const names = result.pins.map(p => p.name).sort();
  assert.deepEqual(names, ['bar', 'baz', 'foo']);
  const foo = result.pins.find(p => p.name === 'foo');
  assert.equal(foo.from, '^4.17.21');
  assert.equal(foo.to, '4.17.21');
  assert.equal(foo.kind, 'dependencies');
  const baz = result.pins.find(p => p.name === 'baz');
  assert.equal(baz.kind, 'devDependencies');
});

test('pinDependencies returns empty pins for already-pinned deps', () => {
  const { pkg, lock } = readFixture('npd-npm-v3-already-pinned');
  const result = napi.pinDependencies({
    packageJson: pkg,
    lockfileContent: lock,
    lockfileType: 'npm',
  });
  assert.deepEqual(result.pins, []);
});

test('pinDependencies works for pnpm lockfiles', () => {
  const dir = resolve(fixturesDir, 'npd-pnpm-v9-unpinned-deps');
  const pkg = readFileSync(resolve(dir, 'package.json'), 'utf8');
  const lock = readFileSync(resolve(dir, 'pnpm-lock.yaml'), 'utf8');
  const result = napi.pinDependencies({
    packageJson: pkg,
    lockfileContent: lock,
    lockfileType: 'pnpm',
  });
  const baz = result.pins.find(p => p.name === 'baz');
  // peer suffix `(qux@20.0.0)` must be stripped.
  assert.equal(baz.to, '1.6.0');
});

test('pinDependencies rejects yarn (no string-content path)', () => {
  assert.throws(
    () =>
      napi.pinDependencies({
        packageJson: '{}',
        lockfileContent: '',
        lockfileType: 'yarn',
      }),
    /yarn lockfile parsing requires a directory path/,
  );
});

test('pinDependencies rejects unknown lockfile type', () => {
  assert.throws(
    () =>
      napi.pinDependencies({
        packageJson: '{}',
        lockfileContent: '',
        lockfileType: 'maven',
      }),
    /unknown lockfile type: maven/,
  );
});

console.log(`\n  ${passed} passed, ${failed} failed`);
if (failed > 0) {
  process.exit(1);
}
