# Changelog

## [0.1.4](https://github.com/smarlhens/riri-node-tools/compare/riri-common-v0.1.3...riri-common-v0.1.4) (2026-07-21)


### Chores

* **deps:** bump cargo dependencies ([6c80946](https://github.com/smarlhens/riri-node-tools/commit/6c80946e9151379c37e008d640f76a94d89db8f7))

## [0.1.3](https://github.com/smarlhens/riri-node-tools/compare/riri-common-v0.1.2...riri-common-v0.1.3) (2026-06-23)


### Features

* **ncd:** add npm-check-deprecations tool ([454d943](https://github.com/smarlhens/riri-node-tools/commit/454d94350be8408b2b82e5e0e842772620a19932))

## [0.1.2](https://github.com/smarlhens/riri-node-tools/compare/riri-common-v0.1.1...riri-common-v0.1.2) (2026-06-09)


### Features

* **npd:** resolve yarn versions from lockfile not node_modules scan ([8f4b8ec](https://github.com/smarlhens/riri-node-tools/commit/8f4b8ec52d3b8017e3434e556955454841179333))

## [0.1.1](https://github.com/smarlhens/riri-node-tools/compare/riri-common-v0.1.0...riri-common-v0.1.1) (2026-05-28)


### Features

* add optional sort-package-json support via --sort flag ([caf7f82](https://github.com/smarlhens/riri-node-tools/commit/caf7f82b74fa0e9d06d1adf56572c9cb372b0295))
* add PackageJsonFile to riri-common & apply_engines_update to riri-nce ([40cdd33](https://github.com/smarlhens/riri-node-tools/commit/40cdd33b5af65e1bbe4a0cdd9e9cc3d61dc2e810))
* **common:** shared .npmrc upsert helper for nce engine-strict + npd save-exact ([cf12169](https://github.com/smarlhens/riri-node-tools/commit/cf121693d4e23115e1ccba18f8c8983ad04872f9))
* **npd:** rewrite riri-npd to consume shared crates with npm support ([4291780](https://github.com/smarlhens/riri-node-tools/commit/42917808e53eb319387a2c759df7a5985d686db2))
* **riri-common:** atomic write for PackageJsonFile ([5e0b08d](https://github.com/smarlhens/riri-node-tools/commit/5e0b08d47447980768310f057d7cb6c53bb507f2))
* **riri-common:** implement detect_lockfile & find_package_json using riri-find-up ([77cc8ce](https://github.com/smarlhens/riri-node-tools/commit/77cc8ce0d15554a6a0e63d40c5234354d1c13aa2))
* workspace / monorepo support ([64d33c1](https://github.com/smarlhens/riri-node-tools/commit/64d33c1dc692d39edd2017a48540df84224b023a))


### Bug Fixes

* **nce:** preserve lockfile indentation on engines update ([368d099](https://github.com/smarlhens/riri-node-tools/commit/368d0999014d574fee30aeb4307f6bed4992c004))


### Code Refactoring

* rename riri-types to riri-common ([d42c2f0](https://github.com/smarlhens/riri-node-tools/commit/d42c2f0728fdf6b39b39db962a38d39bb6c0c943))


### Tests

* **riri-common:** PackageJson parsing & PackageJsonFile read/write tests ([2041bf0](https://github.com/smarlhens/riri-node-tools/commit/2041bf0cfad743bbe920967871557f97af7efbcc))


### Continuous Integration

* **release-please:** fix unbumpable workspace.deps baseline ([91130bc](https://github.com/smarlhens/riri-node-tools/commit/91130bc6a37eda575a0f1f6997d1e2f86704e251))
* **release-please:** inline internal deps for plugin changelog rendering ([e9e68e9](https://github.com/smarlhens/riri-node-tools/commit/e9e68e97b9a712676bf31a7f2965e1b3f7909b37))
* **release-please:** wire cargo-workspace plugin for in-graph crates ([32c754c](https://github.com/smarlhens/riri-node-tools/commit/32c754ce56369a03e2f6c0940f54fa81af4d5458))


### Chores

* bump semver to 1.0.28 & tempfile to 3.27.0 ([0dfb107](https://github.com/smarlhens/riri-node-tools/commit/0dfb107048739c7530269c15667d98fc8037b157))
* **deps:** bump cargo dependencies ([441696f](https://github.com/smarlhens/riri-node-tools/commit/441696f1f8356397d1c447194f013f7697577bfe))
* move tempfile & walkdir to workspace dependencies ([8e4b71f](https://github.com/smarlhens/riri-node-tools/commit/8e4b71fa298bb5a9159431d487c004fee857b29d))


### Dependencies

* The following workspace dependencies were updated
  * dependencies
    * riri-find-up bumped from 0.1.0 to 0.1.1
