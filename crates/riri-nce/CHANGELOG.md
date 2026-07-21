# Changelog

## [0.1.5](https://github.com/smarlhens/riri-node-tools/compare/riri-nce-v0.1.4...riri-nce-v0.1.5) (2026-07-21)


### Bug Fixes

* **nce:** make npm engine suggestion idempotent ([29bb7ed](https://github.com/smarlhens/riri-node-tools/commit/29bb7ed0ea7c623da9a13c6940a073570d4c3f54))


### Dependencies

* The following workspace dependencies were updated
  * dependencies
    * riri-common bumped from 0.1.3 to 0.1.4
    * riri-node-lifecycle bumped from 0.1.2 to 0.1.3
    * riri-npm bumped from 0.1.3 to 0.1.4
    * riri-pnpm bumped from 0.1.3 to 0.1.4
    * riri-yarn bumped from 0.1.3 to 0.1.4
    * riri-task-runner bumped from 0.1.1 to 0.1.2

## [0.1.4](https://github.com/smarlhens/riri-node-tools/compare/riri-nce-v0.1.3...riri-nce-v0.1.4) (2026-06-23)


### Performance Improvements

* **semver-range:** byte-level parse fast path & exact-prerelease fix ([d126278](https://github.com/smarlhens/riri-node-tools/commit/d126278ec4eb38a6c990171f30d8650a58f2b2a7))


### Code Refactoring

* **yarn:** gate node_modules scan behind scan feature ([beb4c40](https://github.com/smarlhens/riri-node-tools/commit/beb4c40bbb9cda427be5e595cd82584b41e00d68))


### Dependencies

* The following workspace dependencies were updated
  * dependencies
    * riri-common bumped from 0.1.2 to 0.1.3
    * riri-node-lifecycle bumped from 0.1.1 to 0.1.2
    * riri-npm bumped from 0.1.2 to 0.1.3
    * riri-pnpm bumped from 0.1.2 to 0.1.3
    * riri-semver-range bumped from 0.1.1 to 0.1.2
    * riri-yarn bumped from 0.1.2 to 0.1.3
    * riri-task-runner bumped from 0.1.0 to 0.1.1

## [0.1.3](https://github.com/smarlhens/riri-node-tools/compare/riri-nce-v0.1.2...riri-nce-v0.1.3) (2026-06-09)


### Features

* distribute nce & npd as standalone cli binaries via cargo-dist ([be9f66e](https://github.com/smarlhens/riri-node-tools/commit/be9f66e52f1e96a4ec49d80885ab794943c53021))


### Dependencies

* The following workspace dependencies were updated
  * dependencies
    * riri-common bumped from 0.1.1 to 0.1.2
    * riri-npm bumped from 0.1.1 to 0.1.2
    * riri-pnpm bumped from 0.1.1 to 0.1.2
    * riri-yarn bumped from 0.1.1 to 0.1.2

## [0.1.2](https://github.com/smarlhens/riri-node-tools/compare/riri-nce-v0.1.1...riri-nce-v0.1.2) (2026-06-09)


### Dependencies

* The following workspace dependencies were updated
  * dependencies
    * riri-node-lifecycle bumped from 0.1.0 to 0.1.1

## [0.1.1](https://github.com/smarlhens/riri-node-tools/compare/riri-nce-v0.1.0...riri-nce-v0.1.1) (2026-05-28)


### Features

* add optional sort-package-json support via --sort flag ([caf7f82](https://github.com/smarlhens/riri-node-tools/commit/caf7f82b74fa0e9d06d1adf56572c9cb372b0295))
* add PackageJsonFile to riri-common & apply_engines_update to riri-nce ([40cdd33](https://github.com/smarlhens/riri-node-tools/commit/40cdd33b5af65e1bbe4a0cdd9e9cc3d61dc2e810))
* **cli:** decouple --sort from --update gate ([ad2e029](https://github.com/smarlhens/riri-node-tools/commit/ad2e02941866152313363ca91c16a2ef0b3de1bb))
* **cli:** expand --debug output with per-step tracing ([3003312](https://github.com/smarlhens/riri-node-tools/commit/30033129362abac4c70dfd6f0f6f561aa1a2e667))
* **common:** shared .npmrc upsert helper for nce engine-strict + npd save-exact ([cf12169](https://github.com/smarlhens/riri-node-tools/commit/cf121693d4e23115e1ccba18f8c8983ad04872f9))
* **nce:** add --precision flag for version output format ([fb4232d](https://github.com/smarlhens/riri-node-tools/commit/fb4232d0dc37076143396c504453e197871b25c0))
* **nce:** add --refresh flag wiring upstream fetch into user cache ([6750ca4](https://github.com/smarlhens/riri-node-tools/commit/6750ca486328e479c6b5e949760c53bc97b69c84))
* **nce:** add npm_bump module for engine npm floor derivation ([f6031c9](https://github.com/smarlhens/riri-node-tools/commit/f6031c94994abc52e983f29a40bffb66fab70330))
* **nce:** add policy module for engines.node lifecycle rewrite ([aa4592a](https://github.com/smarlhens/riri-node-tools/commit/aa4592ad1d3c062a35c3e5168133a47777ddeae8))
* **nce:** detect and normalize engine range format ([a6e735b](https://github.com/smarlhens/riri-node-tools/commit/a6e735b82a0d439a058021c7fa732e8cbf3a7834))
* **nce:** expose CLI via NAPI runCli + ship nce bin shim ([203aab5](https://github.com/smarlhens/riri-node-tools/commit/203aab57284983bc3f109cf5d858d1b102a9de70))
* **nce:** extend JSON output with lifecycle/npm_bump + exit 3 unsatisfiable ([964f878](https://github.com/smarlhens/riri-node-tools/commit/964f8785f190227460927b262adc00a91af37419))
* **nce:** integrate lifecycle pass into check_engines pipeline ([3511691](https://github.com/smarlhens/riri-node-tools/commit/35116912004e09726b8dced2ffcf6276546ad4bf))
* **nce:** refuse write on stale lifecycle data, warn at half threshold ([516e498](https://github.com/smarlhens/riri-node-tools/commit/516e498e60cbdeb94f8cd00c5a2a1b94a4ffa2ba))
* **nce:** tighten open lower bound under restrictive policies ([969725f](https://github.com/smarlhens/riri-node-tools/commit/969725f10997510c1e41af062023cd45fb57384b))
* **nce:** wire lifecycle policy + npm bump CLI flags ([ae16c81](https://github.com/smarlhens/riri-node-tools/commit/ae16c816b78d96511b434777e59eb17c51210291))
* **npd:** pnpm catalog support ([1d13d90](https://github.com/smarlhens/riri-node-tools/commit/1d13d904b8b8cf36baff810b247524c005b180d9))
* **riri-nce:** implement CLI with clap, task-runner & comfy-table ([d71eea2](https://github.com/smarlhens/riri-node-tools/commit/d71eea242c84c3e24129b6872a7191ef2bd95a76))
* **riri-nce:** implement compute_engines_constraint & check_engines ([9ca3637](https://github.com/smarlhens/riri-node-tools/commit/9ca363797cb94190eea6637876d3724ff1e96dc4))
* **riri-nce:** wire pnpm lockfile parsing into CLI ([075feb2](https://github.com/smarlhens/riri-node-tools/commit/075feb234f354e87c998627c930b70cde19b84aa))
* **riri-nce:** wire yarn node_modules scanning into CLI ([f28b9a8](https://github.com/smarlhens/riri-node-tools/commit/f28b9a8c931244e7790e7b2a22d44758f395a95c))


### Bug Fixes

* **nce:** preserve lockfile indentation on engines update ([368d099](https://github.com/smarlhens/riri-node-tools/commit/368d0999014d574fee30aeb4307f6bed4992c004))


### Code Refactoring

* move check-engines into riri-nce ([192ce4a](https://github.com/smarlhens/riri-node-tools/commit/192ce4a31939c6cd0bda83bca300e338e33c797a))
* **riri-nce:** merge pnpm check_engines tests into fixtures file ([bdd2be2](https://github.com/smarlhens/riri-node-tools/commit/bdd2be2a6afe4fbaac920974a3efa35f6ed0aa4e))
* **riri-nce:** prefix npm-specific tests with npm_ ([315e84e](https://github.com/smarlhens/riri-node-tools/commit/315e84e8cf5edf636646175386865ed6c84865d0))
* **riri-npd:** inline types & parsing to decouple from riri-types & riri-npm ([9a1bf4b](https://github.com/smarlhens/riri-node-tools/commit/9a1bf4b93984496157ee68c93043a6d13fdf010d))


### Tests

* **nce:** add policy/npm-bump CLI snapshot fixtures ([dcecb28](https://github.com/smarlhens/riri-node-tools/commit/dcecb28be13b9a25035003e66350cdd936c7c53e))
* **nce:** extend CLI coverage for stable/maintenance/wildcard/compound/precision/update ([4b271a0](https://github.com/smarlhens/riri-node-tools/commit/4b271a0f365e0d47de40e3c37c207e0b4310c325))
* **riri-nce:** check_engines cross-parity tests for pnpm ([c1ad2ac](https://github.com/smarlhens/riri-node-tools/commit/c1ad2ac74dc55fabd15022bb9666955534944ef4))
* **riri-nce:** check_engines cross-parity tests for yarn ([8b57052](https://github.com/smarlhens/riri-node-tools/commit/8b5705266df1e5f830e58ec6a7b54357454d65ff))
* **riri-nce:** check_engines fixture tests & unit tests ([5a1b723](https://github.com/smarlhens/riri-node-tools/commit/5a1b7237e6711958c59041847c6bf89d99ebed3c))
* **riri-nce:** CLI snapshot tests & fix quiet mode output ([f12c7e5](https://github.com/smarlhens/riri-node-tools/commit/f12c7e5cb24cf03dbb36041a5f5a513d4702eeeb))
* **riri-nce:** CLI snapshot tests for pnpm ([3f322e2](https://github.com/smarlhens/riri-node-tools/commit/3f322e2dee1bb246de2286c92c3153cff84c999a))
* **riri-nce:** CLI snapshot tests for yarn ([668fa70](https://github.com/smarlhens/riri-node-tools/commit/668fa70cfde8bd2028eeef21602b53ded87a6075))
* **riri-nce:** yarn no-node-modules error CLI test ([9878e34](https://github.com/smarlhens/riri-node-tools/commit/9878e34440e94cf3a226b0d00f229c8197413c26))
* **riri-nce:** yarn up-to-date CLI test ([0e1053e](https://github.com/smarlhens/riri-node-tools/commit/0e1053ee6e39d394e8f2344fa71831e924a0d31d))


### Continuous Integration

* **release-please:** fix unbumpable workspace.deps baseline ([91130bc](https://github.com/smarlhens/riri-node-tools/commit/91130bc6a37eda575a0f1f6997d1e2f86704e251))
* **release-please:** inline internal deps for plugin changelog rendering ([e9e68e9](https://github.com/smarlhens/riri-node-tools/commit/e9e68e97b9a712676bf31a7f2965e1b3f7909b37))
* **release-please:** wire cargo-workspace plugin for in-graph crates ([32c754c](https://github.com/smarlhens/riri-node-tools/commit/32c754ce56369a03e2f6c0940f54fa81af4d5458))


### Chores

* bump semver to 1.0.28 & tempfile to 3.27.0 ([0dfb107](https://github.com/smarlhens/riri-node-tools/commit/0dfb107048739c7530269c15667d98fc8037b157))
* move tempfile & walkdir to workspace dependencies ([8e4b71f](https://github.com/smarlhens/riri-node-tools/commit/8e4b71fa298bb5a9159431d487c004fee857b29d))
* **riri-nce:** remove redundant riri-pnpm dev-dependency ([ca35e59](https://github.com/smarlhens/riri-node-tools/commit/ca35e5913eb5ad59cd07e54cb41d5775273d336b))
* scaffold cargo workspace ([d277472](https://github.com/smarlhens/riri-node-tools/commit/d277472ded24531798692beed5dff2e5d114621f))


### Dependencies

* The following workspace dependencies were updated
  * dependencies
    * riri-common bumped from 0.1.0 to 0.1.1
    * riri-npm bumped from 0.1.0 to 0.1.1
    * riri-pnpm bumped from 0.1.0 to 0.1.1
    * riri-semver-range bumped from 0.1.0 to 0.1.1
    * riri-yarn bumped from 0.1.0 to 0.1.1
