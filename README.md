# Node.js tools written in Rust

[![GitHub CI][github-ci-shield]][github-ci]
[![GitHub license][license-shield]][license]
[![prek][prek-shield]][prek]

---

## Table of Contents

- [Development][development]
- [TODO][todo]

---

## Development

### Prerequisites

- [rustc][rustc] **>=1.85.0 <2.0.0** (_tested with 1.94.1_)
- [prek][prek] **>=0.3.8** (_tested with 0.3.8_)

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
[prek]: https://prek.j178.dev/
[prek-shield]: https://img.shields.io/endpoint?url=https://raw.githubusercontent.com/j178/prek/master/docs/assets/badge-v0.json
[rustc]: https://www.rust-lang.org/tools/install
[license]: https://github.com/smarlhens/riri-node-tools
[license-shield]: https://img.shields.io/badge/license-BlueOak--1.0.0-blue
[github-ci]: https://github.com/smarlhens/riri-node-tools/actions/workflows/ci.yml
[github-ci-shield]: https://github.com/smarlhens/riri-node-tools/workflows/ci/badge.svg
