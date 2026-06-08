# semver-range benchmarks

## Microbenchmarks — Rust (criterion)

| Benchmark                                  |     Time |
| :----------------------------------------- | -------: |
| nodejs-semver\_ parse range                |   3.3 µs |
| nodejs-semver\_ satisfies                  |  30.0 ns |
| riri\_ parse + restrictive_range (8 pairs) |   5.7 µs |
| riri\_ parse range                         |   2.3 µs |
| riri\_ restrictive_range (8 pairs)         | 682.0 ns |
| riri\_ satisfies                           |  24.0 ns |

## Cross-library (riri vs nodejs-semver)

| Metric      |    riri | nodejs-semver | Speedup |
| :---------- | ------: | ------------: | ------: |
| parse range |  2.3 µs |        3.3 µs |   1.41x |
| satisfies   | 24.0 ns |       30.0 ns |   1.25x |
