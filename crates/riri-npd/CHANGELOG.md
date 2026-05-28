# Changelog

## [0.1.1](https://github.com/smarlhens/riri-node-tools/compare/riri-npd-v0.1.0...riri-npd-v0.1.1) (2026-05-28)


### Features

* **cli:** decouple --sort from --update gate ([ad2e029](https://github.com/smarlhens/riri-node-tools/commit/ad2e02941866152313363ca91c16a2ef0b3de1bb))
* **cli:** expand --debug output with per-step tracing ([3003312](https://github.com/smarlhens/riri-node-tools/commit/30033129362abac4c70dfd6f0f6f561aa1a2e667))
* **common:** shared .npmrc upsert helper for nce engine-strict + npd save-exact ([cf12169](https://github.com/smarlhens/riri-node-tools/commit/cf121693d4e23115e1ccba18f8c8983ad04872f9))
* **npd:** expose CLI via NAPI runCli + ship npd bin shim ([f411075](https://github.com/smarlhens/riri-node-tools/commit/f411075ff053b9650e150df2f6bcf7efad266a3c))
* **npd:** pnpm catalog support ([1d13d90](https://github.com/smarlhens/riri-node-tools/commit/1d13d904b8b8cf36baff810b247524c005b180d9))
* **npd:** pnpm resolver via importers + use fake names in fixtures ([f582097](https://github.com/smarlhens/riri-node-tools/commit/f582097facfebd4ba8b9b142e0b8b352462316ec))
* **npd:** rewrite riri-npd to consume shared crates with npm support ([4291780](https://github.com/smarlhens/riri-node-tools/commit/42917808e53eb319387a2c759df7a5985d686db2))
* **npd:** yarn resolver via node_modules scan ([c4176da](https://github.com/smarlhens/riri-node-tools/commit/c4176dae321df3b86d1d61ee0f4dc91388a022a0))
* workspace / monorepo support ([64d33c1](https://github.com/smarlhens/riri-node-tools/commit/64d33c1dc692d39edd2017a48540df84224b023a))


### Code Refactoring

* move pin-dependencies into riri-npd ([7033ea7](https://github.com/smarlhens/riri-node-tools/commit/7033ea77b3aec80456213e79488975d35bba0b79))
* **riri-npd:** inline types & parsing to decouple from riri-types & riri-npm ([9a1bf4b](https://github.com/smarlhens/riri-node-tools/commit/9a1bf4b93984496157ee68c93043a6d13fdf010d))


### Tests

* **npd:** add npm-v3 file: dependency fixture ([304ba98](https://github.com/smarlhens/riri-node-tools/commit/304ba987bd4d17d262f774a1b629cd1e19daf508))
* **npd:** add npm-v3 link: dependency fixture ([1e2964a](https://github.com/smarlhens/riri-node-tools/commit/1e2964a14ecf02ae5ce958c8971374cda5af8b6c))
* **npd:** add yarn berry v2 unpinned-deps fixture ([7f6f2ba](https://github.com/smarlhens/riri-node-tools/commit/7f6f2ba58ee215483a2761495a215285ac5dcb8b))
* **npd:** rstest #[files] auto-discovery of npd-* fixtures ([d866f83](https://github.com/smarlhens/riri-node-tools/commit/d866f83b0636221e769d4bc28978090dfa387192))


### Continuous Integration

* **release-please:** fix unbumpable workspace.deps baseline ([91130bc](https://github.com/smarlhens/riri-node-tools/commit/91130bc6a37eda575a0f1f6997d1e2f86704e251))
* **release-please:** inline internal deps for plugin changelog rendering ([e9e68e9](https://github.com/smarlhens/riri-node-tools/commit/e9e68e97b9a712676bf31a7f2965e1b3f7909b37))
* **release-please:** wire cargo-workspace plugin for in-graph crates ([32c754c](https://github.com/smarlhens/riri-node-tools/commit/32c754ce56369a03e2f6c0940f54fa81af4d5458))


### Chores

* scaffold cargo workspace ([d277472](https://github.com/smarlhens/riri-node-tools/commit/d277472ded24531798692beed5dff2e5d114621f))


### Dependencies

* The following workspace dependencies were updated
  * dependencies
    * riri-common bumped from 0.1.0 to 0.1.1
    * riri-npm bumped from 0.1.0 to 0.1.1
    * riri-pnpm bumped from 0.1.0 to 0.1.1
    * riri-workspace bumped from 0.1.0 to 0.1.1
    * riri-yarn bumped from 0.1.0 to 0.1.1
