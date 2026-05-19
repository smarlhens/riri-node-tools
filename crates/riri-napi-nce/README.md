# NPM check engines

[![CI](https://github.com/smarlhens/riri-node-tools/actions/workflows/ci.yml/badge.svg)](https://github.com/smarlhens/riri-node-tools/actions/workflows/ci.yml)
[![napi-nce](https://github.com/smarlhens/riri-node-tools/actions/workflows/napi-nce.yml/badge.svg)](https://github.com/smarlhens/riri-node-tools/actions/workflows/napi-nce.yml)
![node-current (scoped)](https://img.shields.io/node/v/@smarlhens/npm-check-engines)
[![license](https://img.shields.io/github/license/smarlhens/riri-node-tools)](https://github.com/smarlhens/riri-node-tools/blob/main/LICENSE.md)
[![Conventional Commits](https://img.shields.io/badge/Conventional%20Commits-1.0.0-yellow.svg)](https://conventionalcommits.org)

**npm-check-engines upgrades your package.json node engines constraint to the most restrictive used by your dependencies.**

This package ships a native Rust core via [NAPI-RS](https://napi.rs/) as part of the [riri-node-tools](https://github.com/smarlhens/riri-node-tools) monorepo.

---

## Table of Contents

- [Prerequisites](#prerequisites)
- [Installation](#installation)
- [Usage](#usage)
  - [CLI](#cli)
  - [Node API](#node-api)
- [CLI Options](#cli-options)
- [Debug](#debug)
- [Thanks](#thanks)

---

## Prerequisites

- [Node.js](https://nodejs.org/en/download/) **version `^20.17.0 || ^22.13.0 || >=23.5.0`**

Supported platforms:

| OS      | Arch                      |
|---------|---------------------------|
| Linux   | x64 (glibc/musl), arm64 (glibc/musl) |
| macOS   | x64, arm64                |
| Windows | x64                       |

---

## Installation

Install globally:

```sh
npm install -g @smarlhens/npm-check-engines
```

Or run with [npx](https://docs.npmjs.com/cli/v8/commands/npx):

```sh
npx @smarlhens/npm-check-engines
```

---

## Usage

### CLI

Compute the most restrictive `engines.node` constraint for the project in the current directory based on the lockfile (`package-lock.json`, `yarn.lock`, or `pnpm-lock.yaml`):

```sh
nce
```

Sample output (against `fixtures/nce-policy-supported-eol-bump`):

```text
  node  >=18.0.0  →  ^22.0.0 || ^24.0.0 || ^25.0.0 || >=26.0.0
  npm   *         →  >=10.5.1

  Run nce -u to upgrade package.json.
```

Update `package.json` (and lockfile, when relevant) with the computed ranges:

```sh
nce -u
```

Emit machine-readable JSON:

```sh
nce --json
```

### Node API

```typescript
import { checkEngines } from '@smarlhens/npm-check-engines';

const packageJson = '...'; // stringified package.json
const lockfileContent = '...'; // stringified lockfile

const { computedEngines, changes } = checkEngines({
  packageJson,
  lockfileContent,
  lockfileType: 'npm', // optional: 'npm' | 'yarn' | 'pnpm' — auto-detected when omitted
  filterEngines: ['node'], // optional: restrict which engines to compute
  precision: 'patch', // optional: 'major' | 'minor' | 'patch'
});

console.log(computedEngines); // { node: '^20.17.0 || ^22.13.0 || >=23.5.0', ... }
for (const { engine, from, to } of changes) {
  console.log(`${engine}: "${from}" → "${to}"`);
}
```

Additional helpers exported by the package:

```typescript
import {
  humanizeRange,
  intersects,
  isSubsetOf,
  restrictiveRange,
  satisfies,
  runCli,
} from '@smarlhens/npm-check-engines';
```

- `humanizeRange(input, precision?)` — trim trailing zero components per precision rule
- `intersects(a, b)` — do two ranges overlap
- `isSubsetOf(a, b)` — is `a` fully contained in `b`
- `restrictiveRange(a, b)` — return the most restrictive of two ranges
- `satisfies(range, version)` — does a concrete version satisfy a range
- `runCli(argv)` — run the `nce` CLI in-process; `argv[0]` must be the program name. Returns exit code.

---

## CLI Options

```text
Check and update Node.js engine constraints in package.json based on the dependency tree from the lockfile

Usage: nce [OPTIONS]

Options:
  -q, --quiet
          Silent mode — no output

  -v, --verbose
          Verbose output

  -d, --debug
          Debug mode — detailed logging

  -e, --engines <ENGINES>
          Engine keys to check (e.g. node, npm, yarn). Defaults to all

  -u, --update
          Update package.json (and lockfile) with computed ranges

      --enable-engine-strict
          Create or update .npmrc with engine-strict=true

      --json
          Output results as JSON

      --sort
          Sort package.json keys (sort-package-json conventions); writes the file even without --update or pending changes

      --precision <PRECISION>
          Version precision in output: major (e.g. >=24), minor (e.g. >=24.0), or patch (e.g. >=24.0.0). Trailing .0 components are trimmed accordingly. Non-zero components are never dropped

          Possible values:
          - major: Trim all trailing .0 (minimum 1 component)
          - minor: Trim trailing .0 patch only (minimum 2 components)
          - patch: Always show major.minor.patch
          
          [default: patch]

      --node-policy <NODE_POLICY>
          Node.js lifecycle policy gate for engines.node
          
          [default: supported]
          [possible values: any, stable, supported, lts, maintenance]

      --allow-eol
          Suppress EOL warnings (does not widen the policy)

      --bump-npm
          Bump engines.npm floor to match the lowest node major in range

      --no-bump-npm
          Disable the npm coupling pass

      --npm-precision <NPM_PRECISION>
          Precision applied to the npm bump floor

          Possible values:
          - major: Trim all trailing .0 (minimum 1 component)
          - minor: Trim trailing .0 patch only (minimum 2 components)
          - patch: Always show major.minor.patch
          
          [default: major]

      --refresh
          Fetch fresh lifecycle data from upstream and update the user cache. Continues with the rest of the run after writing the cache

  -h, --help
          Print help (see a summary with '-h')

  -V, --version
          Print version
```

---

## Debug

```sh
nce -d
```

The `-d/--debug` flag enables detailed logging to stderr. No environment variable is required.

<details>
<summary>Sample debug output (against <code>fixtures/nce-policy-supported-eol-bump</code>)</summary>

```text
  ▸ Detecting lockfile......
  ✓ Detected package-lock.json
  ▸ Reading package.json......
  ✓ Read package.json
  ▸ Parsing lockfile......
  ✓ Parsed lockfile
  ▸ Computing engine constraints......
  ✓ Computed engine constraints
  node  >=18.0.0  →  ^22.0.0 || ^24.0.0 || ^25.0.0 || >=26.0.0
  npm   *         →  >=10.5.1

  Run nce -d -u to upgrade package.json.
```

</details>

---

## Thanks

Originally inspired by [npm-check-updates](https://github.com/raineorshine/npm-check-updates).

---
