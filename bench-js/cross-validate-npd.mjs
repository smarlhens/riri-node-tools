//! Cross-validate `riri-napi-npd` (NAPI) against the JS
//! `@smarlhens/npm-pin-dependencies` package on every shared `fixtures/npd-*`
//! input. Exits non-zero on any divergence in the resulting pin set.
//!
//! Yarn v1 fixtures are skipped because the NAPI binding does not yet expose
//! a string-based yarn path (Phase 11 — `pinDependenciesFromPath`).

import { pinDependenciesFromString } from '@smarlhens/npm-pin-dependencies-v0';
import { existsSync, readFileSync, readdirSync } from 'node:fs';
import { createRequire } from 'node:module';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const require = createRequire(import.meta.url);
const __dirname = dirname(fileURLToPath(import.meta.url));
const rootDir = resolve(__dirname, '..');
const napiDir = resolve(rootDir, 'crates/riri-napi-npd');

const nodeFile = readdirSync(napiDir).find(name => name.startsWith('npm-pin-dependencies.') && name.endsWith('.node'));
if (!nodeFile) {
  console.error('No .node binary found. Run `cd crates/riri-napi-npd && npx napi build --platform --release` first.');
  process.exit(1);
}
const napi = require(resolve(napiDir, nodeFile));

const fixturesDir = resolve(rootDir, 'fixtures');

const sortKey = pin => `${pin.name}|${pin.from}|${pin.to}`;
const normalize = pins => [...pins].sort((a, b) => sortKey(a).localeCompare(sortKey(b)));

const runRust = (fixtureDir, lockfileType) => {
  const packageJson = readFileSync(join(fixtureDir, 'package.json'), 'utf8');
  const lockfileName = lockfileType === 'npm' ? 'package-lock.json' : 'pnpm-lock.yaml';
  const lockfileContent = readFileSync(join(fixtureDir, lockfileName), 'utf8');
  const result = napi.pinDependencies({ packageJson, lockfileContent, lockfileType });
  return result.pins.map(p => ({ name: p.name, from: p.from, to: p.to }));
};

const runJs = async (fixtureDir, lockfileType) => {
  const packageJsonString = readFileSync(join(fixtureDir, 'package.json'), 'utf8');
  if (lockfileType !== 'npm') {
    return null;
  }
  const packageLockString = readFileSync(join(fixtureDir, 'package-lock.json'), 'utf8');
  const result = await pinDependenciesFromString({ packageJsonString, packageLockString });
  return (result.versionsToPin ?? []).map(v => ({
    name: v.dependency,
    from: v.version,
    to: v.pinnedVersion,
  }));
};

const classifyFixture = fixtureDir => {
  if (existsSync(join(fixtureDir, 'package-lock.json'))) {
    return 'npm';
  }
  if (existsSync(join(fixtureDir, 'pnpm-lock.yaml'))) {
    return 'pnpm';
  }
  return 'unsupported';
};

const fixtures = readdirSync(fixturesDir)
  .filter(name => name.startsWith('npd-'))
  .sort();

let compared = 0;
let mismatches = 0;
let skipped = 0;

for (const name of fixtures) {
  const dir = join(fixturesDir, name);
  const kind = classifyFixture(dir);
  if (kind === 'unsupported') {
    console.log(`- ${name} (skip: not supported by JS package)`);
    skipped += 1;
    continue;
  }

  const rustPins = runRust(dir, kind);
  const jsPins = await runJs(dir, kind);
  if (jsPins === null) {
    console.log(`- ${name} (skip: ${kind} not supported by JS package)`);
    skipped += 1;
    continue;
  }
  const rustSorted = normalize(rustPins);
  const jsSorted = normalize(jsPins);
  compared += 1;

  if (JSON.stringify(rustSorted) === JSON.stringify(jsSorted)) {
    console.log(`✓ ${name} (${rustSorted.length} pins)`);
  } else {
    console.log(`✗ ${name}`);
    console.log('  rust:', JSON.stringify(rustSorted));
    console.log('  js:  ', JSON.stringify(jsSorted));
    mismatches += 1;
  }
}

console.log(`\n${compared - mismatches}/${compared} fixtures matched, ${mismatches} mismatches, ${skipped} skipped`);
process.exit(mismatches > 0 ? 1 : 0);
