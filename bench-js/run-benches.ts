// bench-js/run-benches.ts
import { execFileSync } from 'node:child_process';
import { mkdirSync, writeFileSync } from 'node:fs';
import { resolve, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

import { collectCriterion, type CriterionRow } from './lib/criterion.ts';
import { detectEnv } from './lib/env.ts';
import { run as runNce } from './lib/tinybench-nce.ts';
import { run as runNpd } from './lib/tinybench-npd.ts';

const here = dirname(fileURLToPath(import.meta.url));
const rootDir = resolve(here, '..');
const outDir = resolve(here, 'bench-results');
mkdirSync(outDir, { recursive: true });

const write = (name: string, data: unknown): void =>
  writeFileSync(resolve(outDir, `${name}.json`), JSON.stringify(data, null, 2));

const cargoBench = (crate: string, bench: string): void => {
  console.log(`cargo bench -p ${crate} --bench ${bench}`);
  execFileSync('cargo', ['bench', '-p', crate, '--bench', bench], { cwd: rootDir, stdio: 'inherit' });
};

// Rust criterion
cargoBench('riri-nce', 'check_engines');
cargoBench('riri-npd', 'pin_dependencies');
for (const b of ['range_parsing', 'range_satisfies', 'range_intersection']) cargoBench('riri-semver-range', b);
const criterion = collectCriterion(resolve(rootDir, 'target/criterion'));
const pick = (re: RegExp): CriterionRow[] => criterion.filter(c => re.test(c.name));

// JS tinybench
const nceTb = await runNce();
const npdTb = await runNpd();

// Criterion flattens names as `<group>_ <bench>`; partition by group prefix.
write('env', await detectEnv());
write('nce', {
  tool: 'nce',
  criterion: pick(/^check_engines|^parse npm lockfile/),
  tinybench: nceTb,
  cli: [],
  memory: null,
  size: [],
});
write('npd', { tool: 'npd', criterion: pick(/^pin_dependencies/), tinybench: npdTb, cli: [], memory: null, size: [] });
write('semver-range', {
  tool: 'semver-range',
  criterion: pick(/^riri_|^nodejs-semver_/),
  tinybench: [],
  cli: [],
  memory: null,
  size: [],
});
console.log(`\nArtifacts written to ${outDir}`);
