# Changelog

## [0.1.2](https://github.com/smarlhens/riri-node-tools/compare/riri-npm-v0.1.1...riri-npm-v0.1.2) (2026-06-09)


### Dependencies

* The following workspace dependencies were updated
  * dependencies
    * riri-common bumped from 0.1.1 to 0.1.2

## [0.1.1](https://github.com/smarlhens/riri-node-tools/compare/riri-npm-v0.1.0...riri-npm-v0.1.1) (2026-05-28)


### Features

* **npd:** rewrite riri-npd to consume shared crates with npm support ([4291780](https://github.com/smarlhens/riri-node-tools/commit/42917808e53eb319387a2c759df7a5985d686db2))
* **riri-npm:** add version, resolved & link fields to NpmLockEntry ([d6d1e4d](https://github.com/smarlhens/riri-node-tools/commit/d6d1e4d348c591914bf1c778205d8c881090c7b2))
* **riri-npm:** impl LockfileEngines for NpmPackageLock ([0d5a75c](https://github.com/smarlhens/riri-node-tools/commit/0d5a75c057198e861452fe7aa6bb0f8e91ff8313))
* **riri-npm:** implement npm package-lock.json parsing (v1/v2/v3) ([2665b8f](https://github.com/smarlhens/riri-node-tools/commit/2665b8f08488446223101d4cf66fcc5834249cb8))
* **riri-types:** define LockfileEngines trait, Engines enum & EngineConstraintKey ([4e22602](https://github.com/smarlhens/riri-node-tools/commit/4e22602d95f0ef982f2a82dadece1411c85f0f7f))


### Code Refactoring

* extract parsers into riri-npm ([cc2df45](https://github.com/smarlhens/riri-node-tools/commit/cc2df456b74c2b206343a122b5019b092aa5bc96))
* rename riri-types to riri-common ([d42c2f0](https://github.com/smarlhens/riri-node-tools/commit/d42c2f0728fdf6b39b39db962a38d39bb6c0c943))


### Tests

* **riri-npm:** fixture parsing, snapshot tests & validation errors ([3df3d02](https://github.com/smarlhens/riri-node-tools/commit/3df3d025c2a088d0e149e94084bbe8abf8931e39))


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
