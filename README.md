# Node.js tools written in Rust

[![GitHub CI][github-ci-shield]][github-ci]
[![GitHub license][license-shield]][license]
[![prek][prek-shield]][prek]

**Fast, native npm utilities — Rust cores exposed to Node via NAPI-RS that read your lockfile and rewrite `package.json`, much faster than their JavaScript predecessors.**

---

## Table of Contents

- [Tools][tools]
- [Using the crates][crates]
- [Benchmarks][benchmarks]
- [Development][development]
- [License][license-section]

---

## Tools

End-user CLIs published from this monorepo:

| Tool                                                                                                                                                                 | Description                                                                                    | Install                                      |
| -------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------- | -------------------------------------------- |
| [@smarlhens/npm-check-engines](https://npmx.dev/@smarlhens/npm-check-engines) (`nce`, `npm-check-engines`) — [README](crates/riri-napi-nce/README.md)                | Check and update Node.js engine constraints in package.json                                    | `npm i -g @smarlhens/npm-check-engines`      |
| [@smarlhens/npm-check-deprecations](https://npmx.dev/@smarlhens/npm-check-deprecations) (`ncd`, `npm-check-deprecations`) — [README](crates/riri-napi-ncd/README.md) | Find deprecated packages in the lockfile dependency tree and show the chains that pull them in | `npm i -g @smarlhens/npm-check-deprecations` |
| [@smarlhens/npm-pin-dependencies](https://npmx.dev/@smarlhens/npm-pin-dependencies) (`npd`, `npm-pin-dependencies`) — [README](crates/riri-napi-npd/README.md)       | Pin dependency ranges in package.json to the exact versions resolved by the lockfile           | `npm i -g @smarlhens/npm-pin-dependencies`   |

- **npm-check-engines** — npm / pnpm / yarn lockfiles · Node.js lifecycle & EOL policy gates · multiple engine keys (node, npm, yarn) · configurable version precision · JSON output
- **npm-check-deprecations** — npm / yarn / pnpm lockfiles (auto-detected) · dependency chains to each deprecated package · semver-range blocker analysis · newest non-deprecated version hints · JSON output
- **npm-pin-dependencies** — npm / yarn / pnpm lockfiles (auto-detected) · workspace mode · pnpm catalog pinning · save-exact via .npmrc · JSON output

### Supported platforms

Prebuilt native binaries are published for:

| OS      | Architectures                          |
| ------- | -------------------------------------- |
| Linux   | x64 (glibc, musl), arm64 (glibc, musl) |
| macOS   | x64, arm64                             |
| Windows | x64                                    |

---

## Using the crates

The `riri-*` crates are libraries too. They are not on crates.io yet, so depend on them by git, and pin a `rev` rather than a tag: cargo rejects two dependencies on one git URL with different refs, so per-crate tags cannot be combined.

```toml
[dependencies]
riri-nce = { git = "https://github.com/smarlhens/riri-node-tools", rev = "0000000", default-features = false }
riri-ncd = { git = "https://github.com/smarlhens/riri-node-tools", rev = "0000000", default-features = false }
riri-npd = { git = "https://github.com/smarlhens/riri-node-tools", rev = "0000000", default-features = false }
```

`default-features = false` drops the CLI layer, and with it every HTTP client. Both are opt-in: `riri-nce`'s `refresh` (behind `nce --refresh`) and `riri-ncd`'s `http`, the registry-backed `DeprecationSource`; released binaries enable them, and a `cargo install --git` build needs `--features refresh`. Without `http`, `riri-ncd` still analyzes a lockfile: `check_deprecations` takes any `DeprecationSource`. Bring your own, or use the bundled `registry::FileRegistry`, which reads packuments from a directory.

What is supported is what each crate's rustdoc documents. `pub` alone is not a promise: the `cli` modules exist so the binaries and the NAPI shims can share code and move without notice, and undocumented `pub` items are incidental. Nothing reads the filesystem or opens a socket unless its name says so — `PackageJsonFile::read`, `YarnProject::scan`, the `refresh` and `http` features.

Every crate is `0.1.x`, where cargo treats a minor bump as breaking. A breaking change lands as a `feat!:` commit, which release-please turns into a minor bump; everything else becomes a patch.

## Benchmarks

Microbenchmarks (point-in-time, machine-specific) live in [BENCHMARKS.md](BENCHMARKS.md) — per tool: [nce](benchmarks/nce.md), [npd](benchmarks/npd.md), [semver-range](benchmarks/semver-range.md).

---

## Development

### Prerequisites

- [rustc](https://www.rust-lang.org/tools/install) **>=1.88.0 <2.0.0** (_tested with 1.97.1_)
- [prek](https://prek.j178.dev/) **>=0.3.8** (_tested with 0.4.10_)
- [node](https://nodejs.org/) **^22.22.2 || ^24.15.0 || >=26.0.0** (_tested with 22.22.2_)

### Installation

1. Clone the git repository

   ```bash
   git clone https://github.com/smarlhens/riri-node-tools.git
   ```

2. Go into the project directory

   ```bash
   cd riri-node-tools/
   ```

3. Checkout working branch

   ```bash
   git checkout <branch>
   ```

4. Enable pre-commit hooks

   ```bash
   prek install
   ```

---

## License

[BlueOak Model License 1.0.0](LICENSE.md).

[tools]: #tools
[crates]: #using-the-crates
[benchmarks]: #benchmarks
[development]: #development
[license-section]: #license
[prek]: https://prek.j178.dev/
[prek-shield]: https://img.shields.io/endpoint?url=https://raw.githubusercontent.com/j178/prek/master/docs/assets/badge-v0.json
[license]: https://github.com/smarlhens/riri-node-tools
[license-shield]: https://img.shields.io/badge/license-BlueOak--1.0.0-blue
[github-ci]: https://github.com/smarlhens/riri-node-tools/actions/workflows/ci.yml
[github-ci-shield]: https://github.com/smarlhens/riri-node-tools/workflows/ci/badge.svg
