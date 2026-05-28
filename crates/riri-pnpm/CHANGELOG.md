# Changelog

## 0.1.0 (2026-05-28)


### Features

* **npd:** pnpm catalog support ([1d13d90](https://github.com/smarlhens/riri-node-tools/commit/1d13d904b8b8cf36baff810b247524c005b180d9))
* **npd:** pnpm resolver via importers + use fake names in fixtures ([f582097](https://github.com/smarlhens/riri-node-tools/commit/f582097facfebd4ba8b9b142e0b8b352462316ec))
* **riri-pnpm:** add v6 format support (pnpm v8) ([55ec43d](https://github.com/smarlhens/riri-node-tools/commit/55ec43d4db1a14525e991802c57839fab868daba))
* **riri-pnpm:** add v9 format support (pnpm v9/v10) ([218e812](https://github.com/smarlhens/riri-node-tools/commit/218e81218c8a9508e3b3642581acbcdd71a527c6))
* **riri-pnpm:** handle v11 multi-document YAML ([32d0d42](https://github.com/smarlhens/riri-node-tools/commit/32d0d42cada1eb58fa70491bd9eedae360860f00))
* **riri-pnpm:** implement v5 format parser & LockfileEngines ([3432c35](https://github.com/smarlhens/riri-node-tools/commit/3432c356690982c5ed8f3d63905da6e05e5f54ff))
* workspace / monorepo support ([64d33c1](https://github.com/smarlhens/riri-node-tools/commit/64d33c1dc692d39edd2017a48540df84224b023a))


### Code Refactoring

* **riri-pnpm:** migrate from serde_yml to serde-saphyr ([a912ef5](https://github.com/smarlhens/riri-node-tools/commit/a912ef52adce7e4970709b6c3c0592befee705a9))


### Documentation

* update rustc version in readme ([5e9935d](https://github.com/smarlhens/riri-node-tools/commit/5e9935d4ff30d58a8dd5799b7fd7bf0f621d8690))


### Tests

* replace real package names with fake names in pnpm fixtures ([683fb34](https://github.com/smarlhens/riri-node-tools/commit/683fb34130a80be84cdf990265377b11bad22aa3))
* **riri-pnpm:** add v7, v8 & v10 format fixtures ([51db74b](https://github.com/smarlhens/riri-node-tools/commit/51db74b06c86f160519a21d6b7f36b296a6f71ce))
* **riri-pnpm:** cross-parity computation fixtures ([c82be68](https://github.com/smarlhens/riri-node-tools/commit/c82be68228f8d157cc2e8c4b0c6e85a0a6a9bf66))
* **riri-pnpm:** parse error & edge case unit tests ([7b84204](https://github.com/smarlhens/riri-node-tools/commit/7b8420474f4ac4134cbe4265dc5199dd74c97772))
* **riri-pnpm:** v5 fixture & parse snapshot test ([5e02d92](https://github.com/smarlhens/riri-node-tools/commit/5e02d9290ae578604aaa6b829608fad62853d198))


### Continuous Integration

* **release-please:** wire cargo-workspace plugin for in-graph crates ([32c754c](https://github.com/smarlhens/riri-node-tools/commit/32c754ce56369a03e2f6c0940f54fa81af4d5458))


### Chores

* **deps:** bump cargo dependencies ([441696f](https://github.com/smarlhens/riri-node-tools/commit/441696f1f8356397d1c447194f013f7697577bfe))
* **deps:** bump serde-saphyr to 0.0.26 ([d1d0b0c](https://github.com/smarlhens/riri-node-tools/commit/d1d0b0c99914a7985bba367d4fc42bd8f77a9e5c))
* **riri-pnpm:** add serde, serde_yml & thiserror deps ([82c7c86](https://github.com/smarlhens/riri-node-tools/commit/82c7c86f10fdcf022aec07bf5d0f1f5e738f1cad))
* scaffold cargo workspace ([d277472](https://github.com/smarlhens/riri-node-tools/commit/d277472ded24531798692beed5dff2e5d114621f))
* update prek hooks ([d199d14](https://github.com/smarlhens/riri-node-tools/commit/d199d14a25d4db3e42164a1ce878486440f1d6d7))
* update rust crates ([38c586d](https://github.com/smarlhens/riri-node-tools/commit/38c586d4a78b10d66591d2343bf833220e76ea6c))
