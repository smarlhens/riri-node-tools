# NPM check deprecations

[![CI](https://github.com/smarlhens/riri-node-tools/actions/workflows/ci.yml/badge.svg)](https://github.com/smarlhens/riri-node-tools/actions/workflows/ci.yml)
[![napi-ncd](https://github.com/smarlhens/riri-node-tools/actions/workflows/napi-ncd.yml/badge.svg)](https://github.com/smarlhens/riri-node-tools/actions/workflows/napi-ncd.yml)
![node-current (scoped)](https://img.shields.io/node/v/@smarlhens/npm-check-deprecations)
[![license](https://img.shields.io/github/license/smarlhens/riri-node-tools)](https://github.com/smarlhens/riri-node-tools/blob/main/LICENSE.md)
[![Conventional Commits](https://img.shields.io/badge/Conventional%20Commits-1.0.0-yellow.svg)](https://conventionalcommits.org)

**npm-check-deprecations finds deprecated packages anywhere in your lockfile dependency tree and shows the chains that pull them in.**

This package ships a native Rust core via [NAPI-RS](https://napi.rs/) as part of the [riri-node-tools](https://github.com/smarlhens/riri-node-tools) monorepo.

---

## Table of Contents

- [Prerequisites](#prerequisites)
- [Installation](#installation)
- [Usage](#usage)
  - [CLI](#cli)
  - [Node API](#node-api)
- [Options](#options)
- [Workspace mode](#workspace-mode)
- [Debug](#debug)

---

## Prerequisites

- [Node.js](https://nodejs.org/en/download/) **version `^22.22.2 || ^24.15.0 || >=26.0.0`**

Supported platforms:

| OS      | Arch                                   |
| ------- | -------------------------------------- |
| Linux   | x64 (glibc, musl), arm64 (glibc, musl) |
| macOS   | x64, arm64                             |
| Windows | x64                                    |

---

## Installation

Install globally:

```sh
npm install -g @smarlhens/npm-check-deprecations
```

Or run with [npx](https://docs.npmjs.com/cli/v8/commands/npx):

```sh
npx @smarlhens/npm-check-deprecations
```

---

## Usage

### CLI

Find deprecated packages in the dependency tree and print the chains that reach them:

```sh
ncd
```

Sample output (against `fixtures/ncd-npm-deprecated-demo`):

```text
ncd-demo
├─ fake-app@2.4.1  ⛔ blocks: requires fake-legacy@~0.2.0, fix needs 0.3.0 → fake-app update required  (latest: 3.1.0)
│  └─ fake-legacy@0.2.1  ⚠ deprecated: fake-legacy is unmaintained; migrate to fake-modern  (latest: 0.3.0)
├─ fake-test@3.5.0 (dev)  ⛔ blocks: requires fake-legacy@^0.2.0, fix needs 0.3.0 → fake-test update required  (latest: 4.0.0)
│  └─ fake-legacy@0.2.1  ⚠ deprecated: fake-legacy is unmaintained; migrate to fake-modern (see above)
└─ fake-util@1.2.0  (fix: update fake-util — ^1.0.0 allows 1.4.0)  ⚠ deprecated: fake-util is deprecated in favor of @demo/fake-util  (latest: 2.0.0)


  2 deprecated package(s) found
```

Each deprecated package is annotated with its registry deprecation message and the newest non-deprecated version. When a parent's declared range blocks the fix, the parent edge is flagged as a blocker.

Emit machine-readable JSON:

```sh
ncd --json
```

Override the registry (defaults to `.npmrc`, then the public npm registry):

```sh
ncd --registry https://registry.npmjs.org
```

Supports `package-lock.json` (v1/v2/v3), `yarn.lock` (classic & berry), and `pnpm-lock.yaml` (v5/v6/v9), auto-detected.

Exit codes: `0` no deprecated packages · `1` deprecated packages found · `2` runtime error.

### Node API

```typescript
import { checkDeprecations } from '@smarlhens/npm-check-deprecations';

const packageJson = '...'; // stringified package.json
const lockfileContent = '...'; // stringified lockfile

// Fetches packuments from the registry (blocking I/O).
const { tree, deprecated } = checkDeprecations({
  packageJson,
  lockfileContent,
  lockfileType: 'npm', // optional: 'npm' | 'yarn' | 'pnpm' (defaults to 'npm')
  // registry: 'https://registry.npmjs.org', // optional registry override
});

for (const pkg of deprecated) {
  console.log(`${pkg.name}@${pkg.version}: ${pkg.message ?? 'deprecated'}`);
}

if (tree) {
  console.log(tree); // the same chains the CLI prints
}
```

`runCli(argv)` is also exported to run the `ncd` CLI in-process; `argv[0]` must be the program name. Returns the exit code.

---

## Options

```text
Core logic for npm-check-deprecations

Usage: ncd [OPTIONS]

Options:
  -q, --quiet                Silent mode — no progress output
  -v, --verbose              Verbose output
  -d, --debug                Debug mode — detailed logging
      --json                 Output results as JSON
      --registry <REGISTRY>  Registry URL override (default: .npmrc, then <https://registry.npmjs.org>)
  -h, --help                 Print help
  -V, --version              Print version
```

---

## Workspace mode

When run from the root of an npm, pnpm, or yarn workspace, `ncd` auto-detects the workspace and analyzes each member's dependency tree against the shared root lockfile. Output is grouped per member; only members that pull in a deprecated package are shown.

---

## Debug

```sh
ncd -d
```

The `-d/--debug` flag enables detailed logging to stderr. No environment variable is required.

<details>
<summary>Sample debug output (against <code>fixtures/ncd-npm-deprecated-demo</code>)</summary>

```text
  ▸ Detecting lockfile......
  ✓ Detected package-lock.json
  ▸ Reading package.json......
  ✓ Read package.json
  ▸ Building dependency graph......
  ✓ Built dependency graph
  ▸ Checking 4 packages against registry......
  ✓ Checked packages against registry
ncd-demo
├─ fake-app@2.4.1  ⛔ blocks: requires fake-legacy@~0.2.0, fix needs 0.3.0 → fake-app update required  (latest: 3.1.0)
│  └─ fake-legacy@0.2.1  ⚠ deprecated: fake-legacy is unmaintained; migrate to fake-modern  (latest: 0.3.0)
├─ fake-test@3.5.0 (dev)  ⛔ blocks: requires fake-legacy@^0.2.0, fix needs 0.3.0 → fake-test update required  (latest: 4.0.0)
│  └─ fake-legacy@0.2.1  ⚠ deprecated: fake-legacy is unmaintained; migrate to fake-modern (see above)
└─ fake-util@1.2.0  (fix: update fake-util — ^1.0.0 allows 1.4.0)  ⚠ deprecated: fake-util is deprecated in favor of @demo/fake-util  (latest: 2.0.0)


  2 deprecated package(s) found
```

</details>

---
