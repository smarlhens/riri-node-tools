// bench-js/affected-crates.ts
//
// CI entry for the pr-bench `changes` gate. Reads the PR diff and the Cargo
// workspace graph, then writes the affected benchable crate set to
// $GITHUB_OUTPUT as `crates` (space-separated, empty when nothing is affected).
import { execFileSync } from 'node:child_process';
import { appendFileSync } from 'node:fs';

import { affectedBenchableCrates, type CargoMetadata } from './lib/affected.ts';

const MAX_BUFFER = 64 * 1024 * 1024;

const base = process.env.BASE_SHA;
const head = process.env.HEAD_SHA;
if (!base || !head) throw new Error('BASE_SHA and HEAD_SHA must be set');

const changedFiles = execFileSync('git', ['diff', '--name-only', base, head], { encoding: 'utf8' })
  .split('\n')
  .map(line => line.trim())
  .filter(Boolean);

const meta: CargoMetadata = JSON.parse(
  execFileSync('cargo', ['metadata', '--no-deps', '--format-version', '1'], {
    encoding: 'utf8',
    maxBuffer: MAX_BUFFER,
  }),
);

const crates = affectedBenchableCrates(changedFiles, meta).join(' ');

console.log(`Changed files:\n${changedFiles.join('\n')}`);
console.log(`Affected benchable crates: '${crates}'`);

if (process.env.GITHUB_OUTPUT) appendFileSync(process.env.GITHUB_OUTPUT, `crates=${crates}\n`);
