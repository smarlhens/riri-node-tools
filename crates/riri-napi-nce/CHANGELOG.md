# Changelog

## [1.2.1](https://github.com/smarlhens/riri-node-tools/compare/@smarlhens/npm-check-engines-v1.2.0...@smarlhens/npm-check-engines-v1.2.1) (2026-05-28)


### Continuous Integration

* **release-please:** inline internal deps for plugin changelog rendering ([e9e68e9](https://github.com/smarlhens/riri-node-tools/commit/e9e68e97b9a712676bf31a7f2965e1b3f7909b37))


### Chores

* **deps:** bump cargo dependencies ([441696f](https://github.com/smarlhens/riri-node-tools/commit/441696f1f8356397d1c447194f013f7697577bfe))
* **deps:** bump npm deps, pre-commit hooks & sync engines ([69643ad](https://github.com/smarlhens/riri-node-tools/commit/69643ad48eae3baf839d382c9d336901c1d9deee))


### Dependencies

* The following workspace dependencies were updated
  * dependencies
    * riri-common bumped from 0.1.0 to 0.1.1
    * riri-nce bumped from 0.1.0 to 0.1.1
    * riri-npm bumped from 0.1.0 to 0.1.1
    * riri-pnpm bumped from 0.1.0 to 0.1.1
    * riri-semver-range bumped from 0.1.0 to 0.1.1
    * riri-yarn bumped from 0.1.0 to 0.1.1

## [1.2.0](https://github.com/smarlhens/riri-node-tools/compare/@smarlhens/npm-check-engines-v1.1.0...@smarlhens/npm-check-engines-v1.2.0) (2026-05-19)


### Features

* **cli:** decouple --sort from --update gate ([ad2e029](https://github.com/smarlhens/riri-node-tools/commit/ad2e02941866152313363ca91c16a2ef0b3de1bb))
* **cli:** expand --debug output with per-step tracing ([3003312](https://github.com/smarlhens/riri-node-tools/commit/30033129362abac4c70dfd6f0f6f561aa1a2e667))


### Chores

* apply prek formatting + strip trailing ws in regen-readme ([e4d2e10](https://github.com/smarlhens/riri-node-tools/commit/e4d2e10d4eb904eb4a054ad282e0a24d9e4f5f7a))
* **napi:** add npm keywords to nce/npd packages ([07fd9c9](https://github.com/smarlhens/riri-node-tools/commit/07fd9c93bb6840d30e2af3475779bd5056300a49))
* **napi:** bump engines.node to supported policy + add engines.npm floor ([397d4e0](https://github.com/smarlhens/riri-node-tools/commit/397d4e0372bcd88ef82f7ef7352c9d0c8874153d))

## [1.1.0](https://github.com/smarlhens/riri-node-tools/compare/@smarlhens/npm-check-engines-v1.0.0...@smarlhens/npm-check-engines-v1.1.0) (2026-05-19)


### Features

* **xtask:** add regen-readme with full tera templates + ci drift check ([1fa85cf](https://github.com/smarlhens/riri-node-tools/commit/1fa85cfe64d1e13f8a5759b5a6b5985933007f4c))


### Bug Fixes

* **napi:** add per-crate readme, drop readme copy from publish job ([09db420](https://github.com/smarlhens/riri-node-tools/commit/09db4207ed990baf8402afdf852cfb8832367aec))
* **napi:** disable gh-release creation in prepublish to skip wrong-tag lookup ([36d01b0](https://github.com/smarlhens/riri-node-tools/commit/36d01b01fa9ae448b90ff02e597a6ede23051c9c))

## [1.0.0](https://github.com/smarlhens/riri-node-tools/compare/@smarlhens/npm-check-engines-v1.0.0-rc.1...@smarlhens/npm-check-engines-v1.0.0) (2026-05-18)


### Features

* **napi:** split into riri-napi-nce + riri-napi-npd, add pinDependencies binding ([5e682bc](https://github.com/smarlhens/riri-node-tools/commit/5e682bc1c329bcb062c56489812dcc39c8ed5f53))
* **nce:** expose CLI via NAPI runCli + ship nce bin shim ([203aab5](https://github.com/smarlhens/riri-node-tools/commit/203aab57284983bc3f109cf5d858d1b102a9de70))


### Continuous Integration

* **napi:** extract reusable workflow + thin nce/npd callers ([eb880bd](https://github.com/smarlhens/riri-node-tools/commit/eb880bd5ed99e3b76991b0ca089088ecc44c2b6c))


### Chores

* **deps:** bump rust deps ([c469203](https://github.com/smarlhens/riri-node-tools/commit/c469203a10ca8213db9eed2b02ff1bd213fe0258))
* **nce:** release 1.0.0 ([fdaef4f](https://github.com/smarlhens/riri-node-tools/commit/fdaef4fdd06ec99c539b090ce06dd91b79f7d148))

## [1.0.0-rc.1](https://github.com/smarlhens/riri-node-tools/compare/v1.0.0-rc.0...v1.0.0-rc.1) (2026-04-09)


### Chores

* **npm:** update deps ([95da1ee](https://github.com/smarlhens/riri-node-tools/commit/95da1ee1182dd31139593e436ebfcfebbe13ce37))

## [1.0.0-rc.0](https://github.com/smarlhens/riri-node-tools/compare/v0.1.0...v1.0.0-rc.0) (2026-04-09)


### Features

* **napi:** scaffold riri-napi crate with check_engines & semver bindings ([909841b](https://github.com/smarlhens/riri-node-tools/commit/909841b6b008dc375395d66e1f5241a43ff5d175))


### Bug Fixes

* **ci:** use rust release-type for release-please, drop cargo-workspace plugin ([35fd2a8](https://github.com/smarlhens/riri-node-tools/commit/35fd2a87fa1a6eac9b69ed5bb5ab6570aac34983))


### Tests

* **napi:** add JS integration tests & NAPI benchmark comparison ([364ffc1](https://github.com/smarlhens/riri-node-tools/commit/364ffc135d6a2720e5be978d398f4c4b73363f17))


### Continuous Integration

* add NAPI 7-target build & test workflow ([5588573](https://github.com/smarlhens/riri-node-tools/commit/5588573f0d9ad8f13723e6100407a04980b7c03a))
