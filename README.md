# Node.js tools written in Rust

[![GitHub CI][github-ci-shield]][github-ci]
[![GitHub license][license-shield]][license]

---

## Table of Contents

- [Development][development]
- [TODO][todo]

---

## Development

### Prerequisites

- [rustc][rustc] **>=1.74.0 <2.0.0** (_tested with 1.83.0_)
- [pre-commit][pre-commit] **>=3.2.0 <5.0.0** (_tested with 4.0.1_)

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
   pre-commit install
   ```

---

## TODO

- [ ] write unit tests
- [ ] dynamic display mode
- [ ] binary alias (npd / npm-pin-dependencies)
- [ ] expose binary inside Docker for multi-stage usage
- [ ] convert to node using wasm
- [ ] benchmark vs TS/JS
- [ ] create GitHub Actions

[development]: #development
[todo]: #todo
[pre-commit]: https://pre-commit.com/#install
[rustc]: https://www.rust-lang.org/tools/install
[license]: https://github.com/smarlhens/riri-node-tools
[license-shield]: https://img.shields.io/github/license/smarlhens/riri-node-tools
[github-ci]: https://github.com/smarlhens/riri-node-tools/actions/workflows/ci.yml
[github-ci-shield]: https://github.com/smarlhens/riri-node-tools/workflows/ci/badge.svg
