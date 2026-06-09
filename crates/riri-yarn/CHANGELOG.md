# Changelog

## [0.1.2](https://github.com/smarlhens/riri-node-tools/compare/riri-yarn-v0.1.1...riri-yarn-v0.1.2) (2026-06-09)


### Features

* **npd:** resolve yarn versions from lockfile not node_modules scan ([8f4b8ec](https://github.com/smarlhens/riri-node-tools/commit/8f4b8ec52d3b8017e3434e556955454841179333))


### Dependencies

* The following workspace dependencies were updated
  * dependencies
    * riri-common bumped from 0.1.1 to 0.1.2

## [0.1.1](https://github.com/smarlhens/riri-node-tools/compare/riri-yarn-v0.1.0...riri-yarn-v0.1.1) (2026-05-28)


### Features

* **npd:** yarn resolver via node_modules scan ([c4176da](https://github.com/smarlhens/riri-node-tools/commit/c4176dae321df3b86d1d61ee0f4dc91388a022a0))
* **riri-yarn:** implement node_modules scanner & LockfileEngines ([3539517](https://github.com/smarlhens/riri-node-tools/commit/3539517c96fbb60cd161fa0ef576757813695db7))


### Tests

* **riri-nce:** yarn no-node-modules error CLI test ([9878e34](https://github.com/smarlhens/riri-node-tools/commit/9878e34440e94cf3a226b0d00f229c8197413c26))
* **riri-yarn:** add yarn v2, v3 & v4 Berry fixtures ([c6aae25](https://github.com/smarlhens/riri-node-tools/commit/c6aae2520ac989f52486c50b9f64f5c8b49cf9f7))
* **riri-yarn:** scan error & edge case unit tests ([49e15ff](https://github.com/smarlhens/riri-node-tools/commit/49e15ff30fd7fa432d3c55d302f5d41c3a6dd408))
* **riri-yarn:** scan fixture snapshot tests ([c0472e7](https://github.com/smarlhens/riri-node-tools/commit/c0472e77767253402d51a44ffdd41eec0c143d86))
* **riri-yarn:** scoped packages fixture ([b464b8d](https://github.com/smarlhens/riri-node-tools/commit/b464b8dc3d13dae322329f8bc666a621e5e07cbd))
* **riri-yarn:** yarn v4 fixture with node, npm & yarn engines ([8988ac2](https://github.com/smarlhens/riri-node-tools/commit/8988ac268bae9a796f8ab1c91a8bb7e93404d1cc))


### Continuous Integration

* **release-please:** fix unbumpable workspace.deps baseline ([91130bc](https://github.com/smarlhens/riri-node-tools/commit/91130bc6a37eda575a0f1f6997d1e2f86704e251))
* **release-please:** inline internal deps for plugin changelog rendering ([e9e68e9](https://github.com/smarlhens/riri-node-tools/commit/e9e68e97b9a712676bf31a7f2965e1b3f7909b37))
* **release-please:** wire cargo-workspace plugin for in-graph crates ([32c754c](https://github.com/smarlhens/riri-node-tools/commit/32c754ce56369a03e2f6c0940f54fa81af4d5458))


### Chores

* **deps:** bump cargo dependencies ([441696f](https://github.com/smarlhens/riri-node-tools/commit/441696f1f8356397d1c447194f013f7697577bfe))
* move tempfile & walkdir to workspace dependencies ([8e4b71f](https://github.com/smarlhens/riri-node-tools/commit/8e4b71fa298bb5a9159431d487c004fee857b29d))
* **riri-yarn:** add walkdir, serde & serde_json deps ([95f69c7](https://github.com/smarlhens/riri-node-tools/commit/95f69c7ed3b581ecfa7888d24ddda70168b12f4c))
* scaffold cargo workspace ([d277472](https://github.com/smarlhens/riri-node-tools/commit/d277472ded24531798692beed5dff2e5d114621f))


### Dependencies

* The following workspace dependencies were updated
  * dependencies
    * riri-common bumped from 0.1.0 to 0.1.1
