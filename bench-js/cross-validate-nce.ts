//! Cross-validate `riri-napi-nce` (NAPI) against the JS
//! `@smarlhens/npm-check-engines` package on every npm fixture.
//! Exits non-zero on any divergence in the computed engines or change set.
//!
//! Only npm fixtures are cross-validated: the JS package has no pnpm/yarn
//! string entry point.

import { checkEnginesFromString } from '@smarlhens/npm-check-engines-v0';
import { readFileSync, readdirSync } from 'node:fs';
import { createRequire } from 'node:module';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const require = createRequire(import.meta.url);
const __dirname = dirname(fileURLToPath(import.meta.url));
const rootDir = resolve(__dirname, '..');
const napiDir = resolve(rootDir, 'crates/riri-napi-nce');

const nodeFile = readdirSync(napiDir).find(name => name.startsWith('npm-check-engines.') && name.endsWith('.node'));
if (!nodeFile) {
  console.error('No .node binary found. Run `cd crates/riri-napi-nce && npx napi build --platform --release` first.');
  process.exit(1);
}
// eslint-disable-next-line @typescript-eslint/no-explicit-any
const napi: any = require(resolve(napiDir, nodeFile));

const fixturesDir = resolve(rootDir, 'fixtures');

interface NceChange {
  engine: string;
  from: string;
  to: string;
}

const sortKey = (change: NceChange): string => `${change.engine}|${change.from}|${change.to}`;
const normalizeChanges = (changes: NceChange[]): NceChange[] =>
  [...changes].sort((a, b) => sortKey(a).localeCompare(sortKey(b)));

const sortObj = (obj: Record<string, unknown>): Record<string, unknown> => {
  const entries = Object.entries(obj).sort(([a], [b]) => a.localeCompare(b));
  return Object.fromEntries(entries);
};

const runRust = (fixtureDir: string) => {
  const packageJson = readFileSync(join(fixtureDir, 'package.json'), 'utf8');
  const lockfileContent = readFileSync(join(fixtureDir, 'package-lock.json'), 'utf8');
  const result = napi.checkEngines({ lockfileContent, lockfileType: 'npm', packageJson });
  return {
    engines: sortObj(result.computedEngines ?? {}),
    changes: normalizeChanges(
      (result.changes ?? []).map((c: { engine: string; from?: string; to?: string }) => ({
        engine: c.engine,
        from: c.from ?? '',
        to: c.to ?? '',
      })),
    ),
  };
};

const runJs = (fixtureDir: string) => {
  const packageJsonString = readFileSync(join(fixtureDir, 'package.json'), 'utf8');
  const packageLockString = readFileSync(join(fixtureDir, 'package-lock.json'), 'utf8');
  const result = checkEnginesFromString({ packageJsonString, packageLockString });
  const pkg = typeof result.packageJson === 'string' ? JSON.parse(result.packageJson) : result.packageJson;
  const enginesObj = pkg.engines ?? {};
  const stringified = Object.fromEntries(
    Object.entries(enginesObj).map(([k, v]) => [k, Array.isArray(v) ? v.join(', ') : v]),
  );
  const changes = (result.enginesRangeToSet ?? []).map(c => ({
    engine: c.engine,
    from: c.range ?? '',
    to: c.rangeToSet ?? '',
  }));
  return {
    engines: sortObj(stringified),
    changes: normalizeChanges(changes),
  };
};

const fixtures = readdirSync(fixturesDir)
  .filter(name => name.startsWith('npm-') && name !== 'npm-v3-500-deps')
  .sort();

let compared = 0;
let mismatches = 0;

for (const name of fixtures) {
  const dir = join(fixturesDir, name);
  const rust = runRust(dir);
  const js = runJs(dir);
  compared += 1;

  if (JSON.stringify(rust) === JSON.stringify(js)) {
    console.log(`✓ ${name} (${rust.changes.length} changes)`);
  } else {
    console.log(`✗ ${name}`);
    console.log('  rust:', JSON.stringify(rust));
    console.log('  js:  ', JSON.stringify(js));
    mismatches += 1;
  }
}

console.log(`\n${compared - mismatches}/${compared} fixtures matched, ${mismatches} mismatches`);
process.exit(mismatches > 0 ? 1 : 0);
