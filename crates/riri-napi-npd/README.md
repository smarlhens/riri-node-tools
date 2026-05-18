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

- [Node.js](https://nodejs.org/en/download/) **version `^20.17.0 || ^22.13.0 || >=23.5.0`**

Supported platforms:

| OS      | Arch                                 |
|---------|--------------------------------------|
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
Usage: npd [OPTIONS]

Options:
  -q, --quiet              Silent mode — no output
  -v, --verbose            Verbose output
  -d, --debug              Debug mode — detailed logging
  -u, --update             Update package.json with pinned versions
      --json               Output results as JSON
      --sort               Sort package.json keys on write (uses sort-package-json conventions)
      --enable-save-exact  Create or update .npmrc with save-exact=true
  -h, --help               Print help
  -V, --version            Print version
```

---

## Debug

```sh
npd -d
```

The `-d/--debug` flag enables detailed logging to stderr. No environment variable is required.

---
