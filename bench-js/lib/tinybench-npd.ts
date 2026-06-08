import { pinDependenciesFromString as pinDepsV0 } from '@smarlhens/npm-pin-dependencies-v0';
import { pinDependencies as pinDepsV1 } from '@smarlhens/npm-pin-dependencies-v1';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

import { fixturePath, loadLocalNapi, runTinybench, type Fixture, type TinybenchRow } from './tinybench.ts';

interface NpdFixture extends Fixture {
  dir: string;
  lockfileType: string;
  packageJsonString: string;
  lockfileString: string;
}

export async function run(): Promise<TinybenchRow[]> {
  const napiLocal = loadLocalNapi('riri-napi-npd', 'npm-pin-dependencies');

  const fixtures: NpdFixture[] = [
    {
      name: 'npm small (3 deps)',
      dir: fixturePath('npd-npm-v3-unpinned-deps'),
      lockfileType: 'npm',
      iterations: 100,
      warmupIterations: 5,
    },
    {
      name: 'npm large (500 deps)',
      dir: fixturePath('npd-npm-v3-500-deps'),
      lockfileType: 'npm',
      iterations: 3,
      warmupIterations: 0,
    },
  ].map(fixture => ({
    ...fixture,
    packageJsonString: readFileSync(resolve(fixture.dir, 'package.json'), 'utf8'),
    lockfileString: readFileSync(resolve(fixture.dir, 'package-lock.json'), 'utf8'),
  }));

  return runTinybench(fixtures, [
    {
      label: 'js v0.x (TS predecessor)',
      run: fx => pinDepsV0({ packageJsonString: fx.packageJsonString, packageLockString: fx.lockfileString }),
    },
    {
      label: 'napi v1.x (published)',
      run: fx =>
        pinDepsV1({
          packageJson: fx.packageJsonString,
          lockfileContent: fx.lockfileString,
          lockfileType: fx.lockfileType,
        }),
    },
    {
      label: 'napi local (unpublished)',
      run: fx =>
        napiLocal.pinDependencies({
          packageJson: fx.packageJsonString,
          lockfileContent: fx.lockfileString,
          lockfileType: fx.lockfileType,
        }),
    },
  ]);
}
