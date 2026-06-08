import { run } from './lib/tinybench-npd.ts';

import type { TinybenchRow } from './lib/tinybench.ts';
const rows = await run();
const byFixture: Record<string, TinybenchRow[]> = {};
for (const r of rows) (byFixture[r.fixture] ??= []).push(r);
for (const [fixture, rs] of Object.entries(byFixture)) {
  console.log(`\n=== ${fixture} ===\n`);
  console.table(
    rs.map(r => ({
      Name: r.variant,
      'avg (ms)': r.avg_ms.toFixed(4),
      'ops/sec': r.ops.toFixed(2),
      'p99 (ms)': r.p99_ms.toFixed(4),
    })),
  );
}
