# NPM pin dependencies

[![CI](https://github.com/smarlhens/riri-node-tools/actions/workflows/ci.yml/badge.svg)](https://github.com/smarlhens/riri-node-tools/actions/workflows/ci.yml)
[![napi-npd](https://github.com/smarlhens/riri-node-tools/actions/workflows/napi-npd.yml/badge.svg)](https://github.com/smarlhens/riri-node-tools/actions/workflows/napi-npd.yml)
![node-current (scoped)](https://img.shields.io/node/v/@smarlhens/npm-pin-dependencies)
[![license](https://img.shields.io/github/license/smarlhens/riri-node-tools)](https://github.com/smarlhens/riri-node-tools/blob/main/LICENSE.md)
[![Conventional Commits](https://img.shields.io/badge/Conventional%20Commits-1.0.0-yellow.svg)](https://conventionalcommits.org)

**npm-pin-dependencies pins your `package.json` dependency ranges to the exact versions resolved by the lockfile.**

This package ships a native Rust core via [NAPI-RS](https://napi.rs/) as part of the [riri-node-tools](https://github.com/smarlhens/riri-node-tools) monorepo.

---

## Table of Contents

- [Prerequisites](#prerequisites)
- [Installation](#installation)
- [Usage](#usage)
  - [CLI](#cli)
  - [Node API](#node-api)
- [Options](#options)
- [Debug](#debug)

---

## Prerequisites

- [Node.js](https://nodejs.org/en/download/) **version `^22.13.0 || ^24.0.0 || ^25.0.0 || >=26.0.0`**

Supported platforms:

| OS      | Arch                                 |
| ------- | ------------------------------------ |
| Linux   | x64 (glibc/musl), arm64 (glibc/musl) |
| macOS   | x64, arm64                           |
| Windows | x64                                  |

---

## Installation

Install globally:

```sh
npm install -g @smarlhens/npm-pin-dependencies
```

Or run with [npx](https://docs.npmjs.com/cli/v8/commands/npx):

```sh
npx @smarlhens/npm-pin-dependencies
```

---

## Usage

### CLI

Show which `package.json` dependency ranges would be pinned to lockfile-resolved versions:

```sh
npd
```

Sample output (against `fixtures/npd-npm-v3-unpinned-deps`):

```text
    bar  ~18.2.0   →  18.2.0
    foo  ^4.17.21  →  4.17.21
    baz  ^1.0.0    →  1.6.0

  Run npd -u to upgrade package.json.
```

Apply the pins to `package.json`:

```sh
npd -u
```

Emit machine-readable JSON:

```sh
npd --json
```

Supports `package-lock.json`, `yarn.lock`, and `pnpm-lock.yaml` (auto-detected).

### Node API

```typescript
import { pinDependencies } from '@smarlhens/npm-pin-dependencies';

const packageJson = '...'; // stringified package.json
const lockfileContent = '...'; // stringified lockfile

const { pins } = pinDependencies({
  packageJson,
  lockfileContent,
  lockfileType: 'npm', // optional: 'npm' | 'yarn' | 'pnpm' — auto-detected when omitted
});

for (const { name, kind, from, to } of pins) {
  console.log(`${kind} ${name}: "${from}" → "${to}"`);
}
```

`runCli(argv)` is also exported to run the `npd` CLI in-process; `argv[0]` must be the program name. Returns exit code.

---

## Options

```text
Pin range-based dependency specifiers to the exact versions resolved by the lockfile

Usage: npd [OPTIONS]

Options:
  -q, --quiet              Silent mode — no output
  -v, --verbose            Verbose output
  -d, --debug              Debug mode — detailed logging
  -u, --update             Update package.json with pinned versions
      --json               Output results as JSON
      --sort               Sort package.json keys (sort-package-json conventions); writes the file even without --update or pending pins
      --enable-save-exact  Create or update .npmrc with save-exact=true
      --pin-catalog        Resolve and report pnpm catalog entries from `pnpm-workspace.yaml`. On `-u`, rewrites the catalog entries in place. Requires a pnpm project
  -h, --help               Print help
  -V, --version            Print version
```

---

## Debug

```sh
npd -d
```

The `-d/--debug` flag enables detailed logging to stderr. No environment variable is required.

<details>
<summary>Sample debug output (against <code>fixtures/npd-npm-v3-unpinned-deps</code>)</summary>

```text
  ▸ Detecting lockfile......
  ✓ Detected package-lock.json
  ▸ Reading package.json......
  ✓ Read package.json
  ▸ Parsing lockfile......
  ✓ Parsed lockfile
  ▸ Computing dependency pins......
    Pin ~18.2.0 → 18.2.0 bucket="dependencies" package=bar
    Pin ^4.17.21 → 4.17.21 bucket="dependencies" package=foo
    Pin ^1.0.0 → 1.6.0 bucket="devDependencies" package=baz
  ✓ Computed dependency pins
    bar  ~18.2.0   →  18.2.0
    foo  ^4.17.21  →  4.17.21
    baz  ^1.0.0    →  1.6.0

  Run npd -d -u to upgrade package.json.
```

</details>

---
