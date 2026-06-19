import { strict as assert } from 'node:assert';
import { readdirSync } from 'node:fs';
import { createRequire } from 'node:module';
import { resolve, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const require = createRequire(import.meta.url);
const __dirname = dirname(fileURLToPath(import.meta.url));

const nodeFile = readdirSync(__dirname).find(f => f.startsWith('npm-check-deprecations.') && f.endsWith('.node'));
if (!nodeFile) {
  console.error('No .node binary found. Run `npx napi build --platform` first.');
  process.exit(1);
}
const napi = require(`./${nodeFile}`);

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

test('runCli with --version exits 0', () => {
  const code = napi.runCli(['ncd', '--version']);
  assert.equal(code, 0);
});

test('runCli with --help exits 0', () => {
  const code = napi.runCli(['ncd', '--help']);
  assert.equal(code, 0);
});

test('runCli with bogus flag exits 2', () => {
  const code = napi.runCli(['ncd', '--definitely-not-a-flag']);
  assert.equal(code, 2);
});

// A dependency-free project has no packages to query, so this exercises the
// exported analyze function end-to-end without touching the network.
test('checkDeprecations returns an empty result for a project with no dependencies', () => {
  const result = napi.checkDeprecations({
    packageJson: JSON.stringify({ name: 'empty', dependencies: {} }),
    lockfileContent: JSON.stringify({ lockfileVersion: 3, packages: { '': {} } }),
    lockfileType: 'npm',
  });
  assert.deepEqual(result.deprecated, []);
  assert.ok(!result.tree);
});

test('checkDeprecations rejects an unknown lockfile type', () => {
  assert.throws(
    () =>
      napi.checkDeprecations({
        packageJson: '{}',
        lockfileContent: '',
        lockfileType: 'maven',
      }),
    /unknown lockfile type: maven/,
  );
});

test('checkDeprecations rejects malformed package.json', () => {
  assert.throws(
    () =>
      napi.checkDeprecations({
        packageJson: 'not json',
        lockfileContent: '{}',
        lockfileType: 'npm',
      }),
    /failed to parse package\.json/,
  );
});

console.log(`\n  ${passed} passed, ${failed} failed`);
if (failed > 0) {
  process.exit(1);
}
