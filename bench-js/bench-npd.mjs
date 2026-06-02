import { pinDependenciesFromString as pinDepsV0 } from '@smarlhens/npm-pin-dependencies-v0';
import { pinDependencies as pinDepsV1 } from '@smarlhens/npm-pin-dependencies-v1';
import { readFileSync, readdirSync } from 'node:fs';
import { createRequire } from 'node:module';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { Bench } from 'tinybench';

const require = createRequire(import.meta.url);
const __dirname = dirname(fileURLToPath(import.meta.url));

// Load the local .node binary — represents the unpublished working-tree code.
const napiDir = resolve(__dirname, '../crates/riri-napi-npd');
const nodeFile = readdirSync(napiDir).find(f => f.startsWith('npm-pin-dependencies.') && f.endsWith('.node'));
if (!nodeFile) {
  console.error('No .node binary found. Run `cd crates/riri-napi-npd && npx napi build --platform --release` first.');
  process.exit(1);
}
const napiLocal = require(resolve(napiDir, nodeFile));
const rootDir = resolve(__dirname, '..');

const fixtures = {
  'npm small (3 deps)': {
    dir: resolve(rootDir, 'fixtures/npd-npm-v3-unpinned-deps'),
    lockfileType: 'npm',
    lockfileName: 'package-lock.json',
    iterations: 100,
    warmupIterations: 5,
  },
  'npm large (500 deps)': {
    dir: resolve(rootDir, 'fixtures/npd-npm-v3-500-deps'),
    lockfileType: 'npm',
    lockfileName: 'package-lock.json',
    iterations: 3,
    warmupIterations: 0,
  },
};

const fixtureData = {};
for (const [name, config] of Object.entries(fixtures)) {
  fixtureData[name] = {
    ...config,
    packageJsonString: readFileSync(resolve(config.dir, 'package.json'), 'utf8'),
    lockfileString: readFileSync(resolve(config.dir, config.lockfileName), 'utf8'),
  };
}

for (const [name, data] of Object.entries(fixtureData)) {
  console.log(`\n=== ${name} ===\n`);

  const bench = new Bench({
    iterations: data.iterations,
    time: 0,
    warmupIterations: data.warmupIterations,
    warmupTime: 0,
  });

  bench.add('js v0.x (TS predecessor)', async () => {
    await pinDepsV0({
      packageJsonString: data.packageJsonString,
      packageLockString: data.lockfileString,
    });
  });

  bench.add('napi v1.x (published)', () => {
    pinDepsV1({
      packageJson: data.packageJsonString,
      lockfileContent: data.lockfileString,
      lockfileType: data.lockfileType,
    });
  });

  bench.add('napi local (unpublished)', () => {
    napiLocal.pinDependencies({
      packageJson: data.packageJsonString,
      lockfileContent: data.lockfileString,
      lockfileType: data.lockfileType,
    });
  });

  await bench.run();

  const results = bench.tasks.map(task => {
    const r = task.result;
    return {
      Name: task.name,
      'avg (ms)': r.latency.mean.toFixed(4),
      'ops/sec': r.throughput.mean.toFixed(2),
      'p99 (ms)': r.latency.p99.toFixed(4),
    };
  });

  console.table(results);
}
