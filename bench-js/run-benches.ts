// bench-js/run-benches.ts
import { execFileSync } from 'node:child_process';
import { mkdirSync, writeFileSync, readFileSync, existsSync } from 'node:fs';
import { resolve, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

import { collectCriterion, type CriterionRow } from './lib/criterion.ts';
import { detectEnv } from './lib/env.ts';
import {
  parseHyperfine,
  parseMemory,
  parseSize,
  runScript,
  type CliRow,
  type MemoryUsage,
  type SizeRow,
} from './lib/shell.ts';
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

const scriptsDir = resolve(rootDir, 'scripts');

// Run a binary-level bench step, returning `fallback` (and a note) if its tool
// (hyperfine / GNU time / cargo) is missing so the report still generates.
const safe = <T>(label: string, fn: () => T, fallback: T): T => {
  try {
    return fn();
  } catch (error) {
    console.warn(`skip ${label}: ${(error as Error).message.split('\n')[0]}`);
    return fallback;
  }
};

// Native-binary benchmarks (CLI latency, peak RSS, on-disk size) for the
// standalone `nce` / `npd` binaries shipped by cargo-dist.
const benchBinary = (tool: string): { cli: CliRow[]; memory: MemoryUsage | null; size: SizeRow[] } => {
  const cli = safe(
    `cli:${tool}`,
    () => {
      const prefix = resolve(outDir, `cli-${tool}`);
      runScript(resolve(scriptsDir, 'bench-cli.sh'), [tool, prefix]);
      const rows: CliRow[] = [];
      for (const fixture of ['small', 'large']) {
        const file = `${prefix}-${fixture}.json`;
        if (existsSync(file)) rows.push(...parseHyperfine(JSON.parse(readFileSync(file, 'utf8')), fixture));
      }
      return rows;
    },
    [] as CliRow[],
  );

  const size = safe(
    `size:${tool}`,
    () => parseSize(runScript(resolve(scriptsDir, 'bench-size.sh'), [tool, '--json'])),
    [] as SizeRow[],
  );

  const memory = safe(
    `memory:${tool}`,
    () => {
      const usage = parseMemory(runScript(resolve(scriptsDir, 'bench-memory.sh'), [tool, '--json']));
      return usage.peak_kb > 0 ? usage : null; // omit when no GNU time is available
    },
    null as MemoryUsage | null,
  );

  return { cli, memory, size };
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
// Native-binary benchmarks for the cargo-dist CLIs.
const nceBin = benchBinary('nce');
const npdBin = benchBinary('npd');

write('env', await detectEnv());
write('nce', {
  tool: 'nce',
  criterion: pick(/^check_engines|^parse npm lockfile/),
  tinybench: nceTb,
  ...nceBin,
});
write('npd', { tool: 'npd', criterion: pick(/^pin_dependencies/), tinybench: npdTb, ...npdBin });
write('semver-range', {
  tool: 'semver-range',
  criterion: pick(/^riri_|^nodejs-semver_/),
  tinybench: [],
  cli: [],
  memory: null,
  size: [],
});
console.log(`\nArtifacts written to ${outDir}`);
