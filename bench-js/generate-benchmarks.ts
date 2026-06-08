// bench-js/generate-benchmarks.ts
import { readFileSync, writeFileSync, existsSync, mkdirSync } from 'node:fs';
import { resolve, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { format } from 'oxfmt';

import { renderTool, renderIndex } from './lib/markdown.ts';

const here = dirname(fileURLToPath(import.meta.url));
const rootDir = resolve(here, '..');
const inDir = resolve(here, 'bench-results');
const outDir = resolve(rootDir, 'benchmarks');
mkdirSync(outDir, { recursive: true });

const readArtifact = (name: string): any => {
  const p = resolve(inDir, `${name}.json`);
  return existsSync(p) ? JSON.parse(readFileSync(p, 'utf8')) : null;
};

// Format generated markdown with the repo's oxfmt config (programmatic API) so
// output matches the pre-commit hook and stays stable across regenerations.
const oxfmtConfig = JSON.parse(readFileSync(resolve(rootDir, '.oxfmtrc.json'), 'utf8'));
const writeFormatted = async (filePath: string, markdown: string): Promise<void> => {
  const { code } = await format(filePath, markdown, oxfmtConfig);
  writeFileSync(filePath, code);
};

const tools = ['nce', 'npd', 'semver-range'];
for (const tool of tools) {
  const artifact = readArtifact(tool) ?? { tool, criterion: [], tinybench: [], cli: [], memory: null, size: [] };
  await writeFormatted(resolve(outDir, `${tool}.md`), renderTool(artifact) + '\n');
}

const env = readArtifact('env') ?? {
  timestamp: 'n/a',
  os: 'n/a',
  arch: process.arch,
  cpu: 'n/a',
  node: 'n/a',
  npm: 'n/a',
  rust: 'n/a',
  cargo: 'n/a',
  tinybench: 'n/a',
};
await writeFormatted(resolve(rootDir, 'BENCHMARKS.md'), renderIndex(env, tools) + '\n');

console.log(`Wrote and formatted benchmarks/*.md and BENCHMARKS.md`);
