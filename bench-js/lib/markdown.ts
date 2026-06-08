// bench-js/lib/markdown.ts
import { markdownTable } from 'markdown-table';

import { crossCompare, type CriterionRow } from './criterion.ts';
import { formatBytes, formatNanoseconds } from './format.ts';

const speedup = (slow: number, fast: number): string => `~${(slow / fast).toFixed(1)}x`;

interface TinybenchRow {
  fixture: string;
  variant: string;
  avg_ms: number;
  ops: number;
  p99_ms: number;
}

interface CliRow {
  fixture: string;
  variant: string;
  mean_s: number;
}

interface MemoryStats {
  peak_kb: number;
  total_kb: number;
}

interface SizeRow {
  binary: string;
  bytes: number;
}

interface ToolArtifact {
  tool: string;
  criterion?: CriterionRow[];
  tinybench?: TinybenchRow[];
  cli?: CliRow[];
  memory?: MemoryStats | null;
  size?: SizeRow[];
}

interface EnvInfo {
  timestamp: string;
  os: string;
  arch: string;
  cpu: string;
  node: string;
  npm: string;
  rust: string;
  cargo: string;
  tinybench: string;
}

type Align = 'l' | 'r' | 'c';

export const mdTable = (headers: string[], rows: string[][], align?: Align[]): string =>
  markdownTable([headers, ...rows], align ? { align } : undefined);

const section = (title: string, body: string): string => (body ? `## ${title}\n\n${body}\n` : '');

const criterionSection = (rows: CriterionRow[]): string =>
  rows.length
    ? mdTable(
        ['Benchmark', 'Time'],
        rows.map(r => [r.name, formatNanoseconds(r.mean_ns)]),
        ['l', 'r'],
      )
    : '';

const crossLibSection = (rows: CriterionRow[]): string => {
  const comparisons = crossCompare(
    rows.map(r => ({ name: r.name, ns: r.mean_ns })),
    ['nodejs-semver'],
  );
  return comparisons.length
    ? mdTable(
        ['Metric', 'riri', 'nodejs-semver', 'Speedup'],
        comparisons.map(c => [
          c.metric,
          formatNanoseconds(c.ownNs),
          formatNanoseconds(c.referenceNs),
          `${c.speedup.toFixed(2)}x`,
        ]),
        ['l', 'r', 'r', 'r'],
      )
    : '';
};

const tinybenchSection = (rows: TinybenchRow[]): string =>
  rows.length
    ? mdTable(
        ['Fixture', 'Variant', 'avg (ms)', 'ops/sec', 'p99 (ms)'],
        rows.map(r => [r.fixture, r.variant, r.avg_ms.toFixed(4), r.ops.toFixed(0), r.p99_ms.toFixed(4)]),
        ['l', 'l', 'r', 'r', 'r'],
      )
    : '';

const cliSection = (rows: CliRow[]): string =>
  rows.length
    ? mdTable(
        ['Fixture', 'Variant', 'mean (s)'],
        rows.map(r => [r.fixture, r.variant, r.mean_s.toFixed(4)]),
        ['l', 'l', 'r'],
      )
    : '';

const memorySection = (mem: MemoryStats | null | undefined): string =>
  mem
    ? mdTable(
        ['Metric', 'Value'],
        [
          ['Peak heap', `${mem.peak_kb} KB`],
          ['Total allocated', `${mem.total_kb} KB`],
        ],
        ['l', 'r'],
      )
    : '';

const sizeSection = (rows: SizeRow[]): string =>
  rows.length
    ? mdTable(
        ['Binary', 'Size'],
        rows.map(r => [r.binary, formatBytes(r.bytes)]),
        ['l', 'r'],
      )
    : '';

// Derive a Rust-vs-JS speedup table from tinybench rows when both variants exist.
const speedupSection = (rows: TinybenchRow[]): string => {
  if (!rows.length) return '';
  const byFixture: Record<string, Record<string, TinybenchRow>> = {};
  for (const r of rows) (byFixture[r.fixture] ??= {})[r.variant] = r;
  const out: string[][] = [];
  for (const [fixture, variants] of Object.entries(byFixture)) {
    const js = Object.values(variants).find(v => v.variant.startsWith('js'));
    const napi = Object.values(variants).find(v => v.variant.includes('(published)'));
    if (js && napi) {
      out.push([
        fixture,
        `${js.avg_ms.toFixed(4)} ms`,
        `${napi.avg_ms.toFixed(4)} ms`,
        speedup(js.avg_ms, napi.avg_ms),
      ]);
    }
  }
  return out.length ? mdTable(['Fixture', 'JS v0.x', 'napi v1.x', 'Speedup'], out, ['l', 'r', 'r', 'r']) : '';
};

export const renderTool = (artifact: ToolArtifact): string => {
  const parts = [
    `# ${artifact.tool} benchmarks\n`,
    section('Microbenchmarks — Rust (criterion)', criterionSection(artifact.criterion ?? [])),
    section('Cross-library (riri vs nodejs-semver)', crossLibSection(artifact.criterion ?? [])),
    section('JS vs napi (tinybench)', tinybenchSection(artifact.tinybench ?? [])),
    section('Rust vs JS speedup', speedupSection(artifact.tinybench ?? [])),
    section('CLI comparison (hyperfine)', cliSection(artifact.cli ?? [])),
    section('Memory (dhat)', memorySection(artifact.memory)),
    section('Binary size', sizeSection(artifact.size ?? [])),
  ];
  return parts.filter(Boolean).join('\n');
};

export const renderIndex = (env: EnvInfo, tools: string[]): string => {
  const envTable = mdTable(
    ['Property', 'Value'],
    [
      ['Date', env.timestamp],
      ['OS', `${env.os} (${env.arch})`],
      ['CPU', env.cpu],
      ['Node.js', env.node],
      ['npm', env.npm],
      ['Rust', env.rust],
      ['Cargo', env.cargo],
      ['tinybench', env.tinybench],
    ],
  );
  const links = tools.map(t => `- [${t}](benchmarks/${t}.md)`).join('\n');
  return [
    '# Benchmark Results\n',
    '> Generated by `npm run bench:report` in `bench-js/`. Point-in-time, machine-specific.\n',
    '## Environment\n',
    envTable + '\n',
    '## How to reproduce\n',
    '```bash\ncd bench-js && npm run bench:report\n```\n',
    '## Results\n',
    links + '\n',
  ].join('\n');
};
