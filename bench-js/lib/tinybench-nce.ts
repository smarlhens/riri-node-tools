import { checkEnginesFromString as checkEnginesV0 } from '@smarlhens/npm-check-engines-v0';
import { checkEngines as checkEnginesV1 } from '@smarlhens/npm-check-engines-v1';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

import { fixturePath, loadLocalNapi, runTinybench, type Fixture, type TinybenchRow } from './tinybench.ts';

interface NceFixture extends Fixture {
  dir: string;
  packageJsonString: string;
  packageLockString: string;
}

export async function run(): Promise<TinybenchRow[]> {
  const napiLocal = loadLocalNapi('riri-napi-nce', 'npm-check-engines');

  const fixtures: NceFixture[] = [
    { name: 'small (7 deps)', dir: fixturePath('npm-v3-or-ranges-node-only'), iterations: 100, warmupIterations: 5 },
    { name: 'large (500 deps)', dir: fixturePath('npm-v3-500-deps'), iterations: 3, warmupIterations: 0 },
  ].map(fixture => ({
    ...fixture,
    packageJsonString: readFileSync(resolve(fixture.dir, 'package.json'), 'utf8'),
    packageLockString: readFileSync(resolve(fixture.dir, 'package-lock.json'), 'utf8'),
  }));

  return runTinybench(fixtures, [
    {
      label: 'js v0.x (TS predecessor)',
      run: fx => checkEnginesV0({ packageJsonString: fx.packageJsonString, packageLockString: fx.packageLockString }),
    },
    {
      label: 'napi v1.x (published)',
      run: fx =>
        checkEnginesV1({
          lockfileContent: fx.packageLockString,
          lockfileType: 'npm',
          packageJson: fx.packageJsonString,
        }),
    },
    {
      label: 'napi local (unpublished)',
      run: fx =>
        napiLocal.checkEngines({
          lockfileContent: fx.packageLockString,
          lockfileType: 'npm',
          packageJson: fx.packageJsonString,
        }),
    },
  ]);
}
