import assert from 'node:assert/strict';
// bench-js/lib/markdown.test.mjs
import { test } from 'node:test';

import { mdTable, renderIndex, renderTool } from './markdown.ts';

test('mdTable renders header, separator, rows', () => {
  const out = mdTable(
    ['A', 'B'],
    [
      ['1', '2'],
      ['3', '4'],
    ],
  );
  assert.equal(out, '| A | B |\n| - | - |\n| 1 | 2 |\n| 3 | 4 |');
});

test('renderTool includes present sections, omits empty ones', () => {
  const md = renderTool({
    tool: 'nce',
    criterion: [{ name: 'check_engines: 7 deps', mean_ns: 14860, stddev_ns: 120 }],
    tinybench: [],
    cli: [],
    memory: null,
    size: [{ binary: 'riri-nce', bytes: 7340032 }],
  });
  assert.match(md, /# nce benchmarks/);
  assert.match(md, /Microbenchmarks/);
  assert.match(md, /14\.9 µs/);
  assert.match(md, /Binary size/);
  assert.match(md, /7\.34 MB/);
  assert.doesNotMatch(md, /tinybench/i); // empty section omitted
});

test('renderTool builds cross-library speedup from riri vs nodejs-semver', () => {
  const md = renderTool({
    tool: 'semver-range',
    criterion: [
      { name: 'riri_ satisfies', mean_ns: 24, stddev_ns: 1 },
      { name: 'nodejs-semver_ satisfies', mean_ns: 48, stddev_ns: 2 },
    ],
    tinybench: [],
    cli: [],
    memory: null,
    size: [],
  });
  assert.match(md, /Cross-library/);
  assert.match(md, /2\.00x/);
});

test('renderIndex lists env + tool links', () => {
  const md = renderIndex(
    {
      timestamp: '2026-06-02T00:00:00Z',
      os: 'macOS',
      arch: 'arm64',
      cpu: 'M1',
      node: '22',
      npm: '10',
      rust: '1.94',
      cargo: '1.94',
      tinybench: '6',
    },
    ['nce', 'npd', 'semver-range'],
  );
  assert.match(md, /## Environment/);
  assert.match(md, /\[nce\]\(benchmarks\/nce\.md\)/);
});
