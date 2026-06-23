# Changelog

## [0.1.2](https://github.com/smarlhens/riri-node-tools/compare/riri-semver-range-v0.1.1...riri-semver-range-v0.1.2) (2026-06-23)


### Performance Improvements

* **semver-range:** byte-level parse fast path & exact-prerelease fix ([d126278](https://github.com/smarlhens/riri-node-tools/commit/d126278ec4eb38a6c990171f30d8650a58f2b2a7))


### Code Refactoring

* **semver-range:** drop unused public Parts re-export ([aa6e3ed](https://github.com/smarlhens/riri-node-tools/commit/aa6e3ede730bcab9d11752fde61bf10fa9d26665))


### Chores

* **deps:** bump nodejs-semver to 5.0.0 ([ba4033b](https://github.com/smarlhens/riri-node-tools/commit/ba4033b18c1ae4c89ef29fd317d99b9a6db24119))

## [0.1.1](https://github.com/smarlhens/riri-node-tools/compare/riri-semver-range-v0.1.0...riri-semver-range-v0.1.1) (2026-05-28)


### Features

* **nce:** add --precision flag for version output format ([fb4232d](https://github.com/smarlhens/riri-node-tools/commit/fb4232d0dc37076143396c504453e197871b25c0))
* **nce:** add policy module for engines.node lifecycle rewrite ([aa4592a](https://github.com/smarlhens/riri-node-tools/commit/aa4592ad1d3c062a35c3e5168133a47777ddeae8))
* **nce:** detect and normalize engine range format ([a6e735b](https://github.com/smarlhens/riri-node-tools/commit/a6e735b82a0d439a058021c7fa732e8cbf3a7834))
* **riri-semver-range:** implement humanize, split_by_major & apply_min_version ([a1477ed](https://github.com/smarlhens/riri-node-tools/commit/a1477edfd7adef342dcecc9d7a92f8f9f2d49c85))
* **riri-semver-range:** implement intersects & is_subset_of ([eb844b6](https://github.com/smarlhens/riri-node-tools/commit/eb844b64e7521f032be66b915cbd3521576518a3))
* **riri-semver-range:** implement range parsing & satisfies ([7eda138](https://github.com/smarlhens/riri-node-tools/commit/7eda138e4c44fecb32df5891ff08f39649f4ccb1))
* **riri-semver-range:** implement restrictive_range ([d21cd14](https://github.com/smarlhens/riri-node-tools/commit/d21cd14b1ba8eb0962e4061c8a06813d77b8d5ab))
* **riri-semver-range:** scaffold crate & define core types ([fad2bb5](https://github.com/smarlhens/riri-node-tools/commit/fad2bb5cb658b9982f812278cb00d32f1439032f))


### Bug Fixes

* **riri-semver-range:** is_caret must not match &gt;=0.0.0 &lt;1.0.0 as ^0.0.0 ([76f533e](https://github.com/smarlhens/riri-node-tools/commit/76f533e9ba15be1e4a68bc1fc6b8ef61103502c3))
* **semver-range:** rewrite restrictive_range with pairwise interval intersection ([bb349e7](https://github.com/smarlhens/riri-node-tools/commit/bb349e754abe1a663e2e44391ec46046b3218e80))


### Performance Improvements

* **riri-semver-range:** add benchmarks ([6bff710](https://github.com/smarlhens/riri-node-tools/commit/6bff710240e33a9eb4fcac12f15dc36b13809ec3))


### Tests

* **riri-semver-range:** port node-semver tests, cross-validation & proptest ([3231d85](https://github.com/smarlhens/riri-node-tools/commit/3231d852f9e4eabae16ba50245e97a80616a4022))


### Continuous Integration

* **release-please:** fix unbumpable workspace.deps baseline ([91130bc](https://github.com/smarlhens/riri-node-tools/commit/91130bc6a37eda575a0f1f6997d1e2f86704e251))
* **release-please:** wire cargo-workspace plugin for in-graph crates ([32c754c](https://github.com/smarlhens/riri-node-tools/commit/32c754ce56369a03e2f6c0940f54fa81af4d5458))


### Chores

* **deps:** bump cargo dependencies ([441696f](https://github.com/smarlhens/riri-node-tools/commit/441696f1f8356397d1c447194f013f7697577bfe))
* scaffold cargo workspace ([d277472](https://github.com/smarlhens/riri-node-tools/commit/d277472ded24531798692beed5dff2e5d114621f))
