import { checkEnginesFromString } from '@smarlhens/npm-check-engines';
// TODO: Add NAPI-RS binding benchmark here once phase 7 is complete
import { readFileSync } from 'node:fs';
import { resolve, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { Bench } from 'tinybench';

const __dirname = dirname(fileURLToPath(import.meta.url));
const rootDir = resolve(__dirname, '..');

const fixtures = {
  'small (7 deps)': {
    dir: resolve(rootDir, 'fixtures/npm-v3-or-ranges-node-only'),
    iterations: 100,
    warmupIterations: 5,
  },
  'large (500 deps)': {
    dir: resolve(rootDir, 'fixtures/npm-v3-500-deps'),
    iterations: 3,
    warmupIterations: 0,
  },
};

// Pre-read fixture files
const fixtureData = {};
for (const [name, config] of Object.entries(fixtures)) {
  fixtureData[name] = {
    ...config,
    packageJsonString: readFileSync(resolve(config.dir, 'package.json'), 'utf8'),
    packageLockString: readFileSync(resolve(config.dir, 'package-lock.json'), 'utf8'),
  };
}

for (const [name, data] of Object.entries(fixtureData)) {
  console.log(`\n=== ${name} ===\n`);

  const bench = new Bench({
    time: 0,
    iterations: data.iterations,
    warmupTime: 0,
    warmupIterations: data.warmupIterations,
  });

  bench.add('js checkEnginesFromString', () => {
    checkEnginesFromString({
      packageJsonString: data.packageJsonString,
      packageLockString: data.packageLockString,
    });
  });

  // TODO: Add NAPI-RS binding benchmark here once phase 7 is complete

  await bench.run();

  const results = bench.tasks.map(task => {
    const r = task.result;
    const avgMs = r.latency.mean.toFixed(4);
    const p99Ms = r.latency.p99.toFixed(4);
    const opsPerSec = r.throughput.mean.toFixed(2);
    return {
      Name: task.name,
      'ops/sec': opsPerSec,
      'avg (ms)': avgMs,
      'p99 (ms)': p99Ms,
    };
  });

  console.table(results);
}
