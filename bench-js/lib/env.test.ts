import assert from 'node:assert/strict';
// bench-js/lib/env.test.ts
import { test } from 'node:test';

import { normalizeEnvinfo } from './env.ts';

test('normalizeEnvinfo flattens envinfo JSON into env fields', () => {
  const info = {
    System: { OS: 'macOS 15.5', CPU: '(10) arm64 Apple M1 Pro' },
    Binaries: { Node: { version: '22.22.2' }, npm: { version: '10.9.7' } },
    npmPackages: { tinybench: { installed: '6.0.2' } },
  };
  const env = normalizeEnvinfo(info);
  assert.equal(env.os, 'macOS 15.5');
  assert.equal(env.cpu, '(10) arm64 Apple M1 Pro');
  assert.equal(env.node, '22.22.2');
  assert.equal(env.npm, '10.9.7');
  assert.equal(env.tinybench, '6.0.2');
});
