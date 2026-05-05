import { strict as assert } from 'node:assert';
import { readFileSync, readdirSync, existsSync } from 'node:fs';
import { createRequire } from 'node:module';
import { resolve, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const require = createRequire(import.meta.url);
const __dirname = dirname(fileURLToPath(import.meta.url));

// Find the .node binary for the current platform
const nodeFile = readdirSync(__dirname).find(f => f.startsWith('npm-check-engines.') && f.endsWith('.node'));
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
    passed++;
    console.log(`  \u2713 ${name}`);
  } catch (error) {
    failed++;
    console.log(`  \u2717 ${name}`);
    console.log(`    ${error.message}`);
  }
};

// ── checkEngines tests ──────────────────────────────────────────

console.log('\ncheckEngines:');

const npmFixtures = readdirSync(fixturesDir).filter(
  d => d.startsWith('npm-') && existsSync(resolve(fixturesDir, d, 'package-lock.json')),
);

for (const fixture of npmFixtures) {
  test(`npm: ${fixture}`, () => {
    const dir = resolve(fixturesDir, fixture);
    const packageJson = readFileSync(resolve(dir, 'package.json'), 'utf8');
    const lockfile = readFileSync(resolve(dir, 'package-lock.json'), 'utf8');

    const result = napi.checkEngines({
      lockfileContent: lockfile,
      lockfileType: 'npm',
      packageJson,
    });

    assert.ok(result.computedEngines, 'should have computedEngines');
    assert.ok(Array.isArray(result.changes), 'should have changes array');
    assert.ok('node' in result.computedEngines, 'should compute node engine');
  });
}

const pnpmFixtures = readdirSync(fixturesDir).filter(
  d => d.startsWith('pnpm-') && existsSync(resolve(fixturesDir, d, 'pnpm-lock.yaml')),
);

for (const fixture of pnpmFixtures) {
  test(`pnpm: ${fixture}`, () => {
    const dir = resolve(fixturesDir, fixture);
    const packageJson = readFileSync(resolve(dir, 'package.json'), 'utf8');
    const lockfile = readFileSync(resolve(dir, 'pnpm-lock.yaml'), 'utf8');

    const result = napi.checkEngines({
      lockfileContent: lockfile,
      lockfileType: 'pnpm',
      packageJson,
    });

    assert.ok(result.computedEngines, 'should have computedEngines');
    assert.ok(Array.isArray(result.changes), 'should have changes array');
  });
}

// ── Semver utility tests ────────────────────────────────────────

console.log('\nsemver utilities:');

test('humanizeRange: caret', () => {
  assert.equal(napi.humanizeRange('^1.2.3'), '^1.2.3');
});

test('humanizeRange: gte', () => {
  assert.equal(napi.humanizeRange('>=16.0.0'), '>=16.0.0');
});

test('humanizeRange: wildcard', () => {
  assert.equal(napi.humanizeRange('*'), '*');
});

test('humanizeRange: with precision major', () => {
  assert.equal(napi.humanizeRange('>=24.0.0', 'major'), '>=24');
});

test('humanizeRange: with precision minor', () => {
  assert.equal(napi.humanizeRange('>=24.0.0', 'minor'), '>=24.0');
});

test('restrictiveRange: basic', () => {
  assert.equal(napi.restrictiveRange('^14.17.0 || ^16.10.0', '>=16.0.0'), '^16.10.0');
});

test('restrictiveRange: disjoint or-range bug fix', () => {
  assert.equal(napi.restrictiveRange('>=5.0.0 <9.0.0', '^5.0.0 || ^11.0.0'), '^5.0.0');
});

test('satisfies: true', () => {
  assert.equal(napi.satisfies('>=16.0.0', '18.0.0'), true);
});

test('satisfies: false', () => {
  assert.equal(napi.satisfies('>=16.0.0', '14.0.0'), false);
});

test('isSubsetOf: true', () => {
  assert.equal(napi.isSubsetOf('^16.0.0', '>=14.0.0'), true);
});

test('isSubsetOf: false', () => {
  assert.equal(napi.isSubsetOf('>=14.0.0', '^16.0.0'), false);
});

test('intersects: true', () => {
  assert.equal(napi.intersects('^14.0.0 || ^16.0.0', '>=16.0.0'), true);
});

test('intersects: false', () => {
  assert.equal(napi.intersects('^14.0.0', '>=16.0.0'), false);
});

// ── Filter engines ──────────────────────────────────────────────

console.log('\nfilter engines:');

test('checkEngines with filterEngines', () => {
  const dir = resolve(fixturesDir, 'npm-v3-or-ranges-node-npm-yarn');
  const packageJson = readFileSync(resolve(dir, 'package.json'), 'utf8');
  const lockfile = readFileSync(resolve(dir, 'package-lock.json'), 'utf8');

  const result = napi.checkEngines({
    filterEngines: ['node'],
    lockfileContent: lockfile,
    lockfileType: 'npm',
    packageJson,
  });

  assert.equal(Object.keys(result.computedEngines).length, 1, 'should only compute node');
  assert.ok('node' in result.computedEngines);
});

// ── Error handling ──────────────────────────────────────────────

console.log('\nerror handling:');

test('checkEngines: invalid package.json throws', () => {
  assert.throws(() => {
    napi.checkEngines({
      lockfileContent: '{}',
      packageJson: 'not json',
    });
  }, /failed to parse package.json/);
});

test('checkEngines: invalid lockfile throws', () => {
  assert.throws(() => {
    napi.checkEngines({
      lockfileContent: 'not json',
      packageJson: '{}',
    });
  }, /failed to parse/);
});

test('humanizeRange: invalid range throws', () => {
  assert.throws(() => {
    napi.humanizeRange('>>>invalid<<<');
  });
});

test('satisfies: invalid version throws', () => {
  assert.throws(() => {
    napi.satisfies('>=16.0.0', 'not-a-version');
  }, /invalid version/);
});

// ── Summary ─────────────────────────────────────────────────────

console.log(`\n${passed} passed, ${failed} failed\n`);
process.exit(failed > 0 ? 1 : 0);
