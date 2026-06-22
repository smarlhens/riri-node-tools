// bench-js/ci-bench.ts
//
// bench-run orchestrator: builds the affected binary crates, records their
// sizes, counts workspace tests, measures peak RSS, and runs the criterion
// benches — then writes `binary-sizes`, `test-count` and `peak-rss-kb` to
// $GITHUB_OUTPUT. Replaces the inline shell of .github/workflows/bench-run.yml.
import { execFileSync, spawnSync } from 'node:child_process';
import { appendFileSync, existsSync, mkdirSync, mkdtempSync, readFileSync, statSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

import {
  BINARIES,
  benchTargets,
  packumentNames,
  parseMaxRssKb,
  parseTestCount,
  resolveCrateSet,
  RSS_TARGETS,
  type CargoMetadataTargets,
} from './lib/ci-bench.ts';

const MAX_BUFFER = 64 * 1024 * 1024;
const rootDir = resolve(dirname(fileURLToPath(import.meta.url)), '..');

// Stream a cargo command (build/bench) to the log, failing the step on error.
const cargoRun = (args: string[]): void => {
  execFileSync('cargo', args, { cwd: rootDir, stdio: 'inherit' });
};
// Capture a cargo command's stdout (metadata/test list).
const cargoCapture = (args: string[]): string =>
  spawnSync('cargo', args, { cwd: rootDir, encoding: 'utf8', maxBuffer: MAX_BUFFER }).stdout;

const baselineName = process.env.BASELINE_NAME ?? 'pr';
const { set, buildPackages } = resolveCrateSet(process.env.CRATES ?? '');
console.log(`Crate set: ${set.join(' ') || '(none)'}`);

// ── Build release binaries ───────────────────────────────────────────
if (buildPackages.length > 0) cargoRun(['build', '--release', ...buildPackages.flatMap(pkg => ['-p', pkg])]);

// ── Record binary sizes ──────────────────────────────────────────────
const binarySizes: Record<string, number> = {};
for (const binary of BINARIES) {
  const path = resolve(rootDir, 'target/release', binary);
  if (set.includes(binary) && existsSync(path)) binarySizes[binary] = statSync(path).size;
}

// ── Count tests ──────────────────────────────────────────────────────
const list = spawnSync('cargo', ['test', '--workspace', '--', '--list'], {
  cwd: rootDir,
  encoding: 'utf8',
  maxBuffer: MAX_BUFFER,
});
const testCount = parseTestCount(`${list.stdout ?? ''}${list.stderr ?? ''}`);

// ── Measure peak RSS ─────────────────────────────────────────────────
// ncd needs a registry; synthesize an offline one (an empty packument per
// locked package) so the run is deterministic against the shared lockfile.
const synthesizeRegistry = (fixtureDir: string): string => {
  const registry = mkdtempSync(join(tmpdir(), 'reg-'));
  for (const name of packumentNames(readFileSync(join(fixtureDir, 'package-lock.json'), 'utf8'))) {
    const file = join(registry, `${name}.json`);
    mkdirSync(dirname(file), { recursive: true });
    writeFileSync(file, '{}');
  }
  return registry;
};

const peakRssKb: Record<string, number> = {};
for (const { binary, fixture, needsRegistry } of RSS_TARGETS) {
  const binaryPath = resolve(rootDir, 'target/release', binary);
  const fixtureDir = resolve(rootDir, fixture);
  if (!set.includes(binary) || !existsSync(binaryPath) || !existsSync(fixtureDir)) continue;
  const args = ['-v', binaryPath, '--quiet'];
  if (needsRegistry) args.push('--registry', `file://${synthesizeRegistry(fixtureDir)}`);
  const measured = spawnSync('/usr/bin/time', args, { cwd: fixtureDir, encoding: 'utf8', maxBuffer: MAX_BUFFER });
  peakRssKb[binary] = parseMaxRssKb(measured.stderr ?? '');
}

// ── Run benchmarks ───────────────────────────────────────────────────
const meta: CargoMetadataTargets = JSON.parse(cargoCapture(['metadata', '--no-deps', '--format-version', '1']));
const targets = benchTargets(set, meta);
console.log(`Bench targets: ${targets.join(' ') || '(none)'}`);
if (targets.length > 0) {
  cargoRun(['bench', ...targets.flatMap(target => ['--bench', target]), '--', '--save-baseline', baselineName]);
}

// ── Emit outputs ─────────────────────────────────────────────────────
if (process.env.GITHUB_OUTPUT) {
  appendFileSync(
    process.env.GITHUB_OUTPUT,
    `binary-sizes=${JSON.stringify(binarySizes)}\ntest-count=${testCount}\npeak-rss-kb=${JSON.stringify(peakRssKb)}\n`,
  );
}
