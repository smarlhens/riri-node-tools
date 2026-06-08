// bench-js/lib/tinybench.ts
// Shared tinybench harness for the per-tool benchmarks (nce, npd). Each tool
// supplies pre-read fixtures and a list of variants; this runs one Bench per
// fixture and returns flat result rows consumed by lib/markdown.mjs.
import { readdirSync } from 'node:fs';
import { createRequire } from 'node:module';
import { resolve, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { Bench } from 'tinybench';

const require = createRequire(import.meta.url);
const rootDir = resolve(dirname(fileURLToPath(import.meta.url)), '../..');

export interface Fixture {
  name: string;
  iterations: number;
  warmupIterations: number;
  [key: string]: unknown;
}

export interface Variant<F extends Fixture = Fixture> {
  label: string;
  run: (fixture: F) => unknown | Promise<unknown>;
}

export interface TinybenchRow {
  fixture: string;
  variant: string;
  avg_ms: number;
  ops: number;
  p99_ms: number;
}

export const fixturePath = (name: string): string => resolve(rootDir, 'fixtures', name);

// Load a crate's local .node binary — the unpublished working-tree code.
// eslint-disable-next-line @typescript-eslint/no-explicit-any
export const loadLocalNapi = (crate: string, binaryPrefix: string): any => {
  const napiDir = resolve(rootDir, 'crates', crate);
  const nodeFile = readdirSync(napiDir).find(f => f.startsWith(`${binaryPrefix}.`) && f.endsWith('.node'));
  if (!nodeFile) {
    console.error(`No .node binary found. Run \`cd crates/${crate} && npx napi build --platform --release\` first.`);
    process.exit(1);
  }
  return require(resolve(napiDir, nodeFile));
};

// fixtures: [{ name, iterations, warmupIterations, ... }]
// variants: [{ label, run: fixture => unknown | Promise }]
export const runTinybench = async <F extends Fixture>(
  fixtures: F[],
  variants: Variant<F>[],
): Promise<TinybenchRow[]> => {
  const rows: TinybenchRow[] = [];
  for (const fixture of fixtures) {
    const bench = new Bench({
      iterations: fixture.iterations,
      time: 0,
      warmupIterations: fixture.warmupIterations,
      warmupTime: 0,
    });
    for (const variant of variants) {
      bench.add(variant.label, () => variant.run(fixture));
    }
    await bench.run();
    for (const task of bench.tasks) {
      const r = task.result as unknown as { latency: { mean: number; p99: number }; throughput: { mean: number } };
      rows.push({
        fixture: fixture.name,
        variant: task.name,
        avg_ms: r.latency.mean,
        ops: r.throughput.mean,
        p99_ms: r.latency.p99,
      });
    }
  }
  return rows;
};
