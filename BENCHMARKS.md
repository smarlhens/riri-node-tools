# Benchmark Results

> These results are point-in-time measurements. Run benchmarks locally for up-to-date numbers.

## Environment

| Property   | Value                                |
| ---------- | ------------------------------------ |
| Date       | 2026-04-07                           |
| OS         | Windows 11 Pro N 10.0.26200 (x86_64) |
| CPU        | AMD Ryzen 5 5600X 6-Core Processor   |
| Rust       | rustc 1.94.1 (e408947bf 2026-03-25)  |
| Cargo      | cargo 1.94.1 (29ea6fb6a 2026-03-24)  |
| Node.js    | v24.14.1                             |
| JS package | @smarlhens/npm-check-engines@0.14.4  |
| tinybench  | 6.0.0                                |

## How to reproduce

### Rust microbenchmarks (criterion)

```bash
cargo bench -p riri-semver-range
cargo bench -p riri-nce --bench check_engines
```

### JS microbenchmarks (tinybench)

```bash
cd bench-js && npm install && npm run bench
```

### CLI comparison (hyperfine)

```bash
cargo build --release -p riri-nce
./scripts/bench-cli.sh
```

### Memory profiling (dhat)

```bash
cargo test -p riri-nce --test memory_profile -- --ignored --nocapture
```

### Binary size

```bash
cargo build --release -p riri-nce
./scripts/bench-size.sh
```

## Results

### Microbenchmarks - Rust (criterion)

| Benchmark                           | Time      |
| ----------------------------------- | --------- |
| restrictive_range (8 pairs)         | 1.12 us   |
| parse + restrictive_range (8 pairs) | 12.40 us  |
| check_engines: 7 deps               | 14.86 us  |
| check_engines: 500 deps             | 253.39 us |
| parse npm lockfile: 500 deps        | 749.87 us |

### Microbenchmarks - JS (tinybench 6.0.0, Node.js v24.14.1)

| Benchmark                        | ops/sec | avg (ms) | p99 (ms) |
| -------------------------------- | ------- | -------- | -------- |
| checkEnginesFromString: 7 deps   | 4,991   | 0.256    | 0.809    |
| checkEnginesFromString: 500 deps | 311     | 3.426    | 4.237    |

### Rust vs JS comparison

| Fixture                  | Rust     | JS       | Speedup |
| ------------------------ | -------- | -------- | ------- |
| 7 deps (check_engines)   | 14.86 us | 256 us   | ~17x    |
| 500 deps (check_engines) | 253 us   | 3,426 us | ~14x    |

> **Note:** The JS library does not support bounded ranges like `>=14.0.0 <22.0.0` (causes infinite loop in `restrictiveRange`). The 500-dep fixture avoids this pattern to allow a fair comparison. Real-world projects using such ranges would see even larger speedups with Rust.

### Memory (dhat heap profiling, 500-dep fixture)

| Metric                | Full check_engines      |
| --------------------- | ----------------------- |
| Peak heap (at t-gmax) | 715 KB (5,115 blocks)   |
| Total allocated       | 1,050 KB (9,675 blocks) |

### Binary size (release build, x86_64-pc-windows-gnu)

| Binary       | Size   |
| ------------ | ------ |
| riri-nce.exe | 7.3 MB |
