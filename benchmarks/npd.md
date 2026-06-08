# npd benchmarks

## Microbenchmarks — Rust (criterion)

| Benchmark                  |     Time |
| :------------------------- | -------: |
| pin*dependencies* 3 deps   | 261.0 ns |
| pin*dependencies* 500 deps |  45.8 µs |

## JS vs napi (tinybench)

| Fixture              | Variant                  | avg (ms) | ops/sec | p99 (ms) |
| :------------------- | :----------------------- | -------: | ------: | -------: |
| npm small (3 deps)   | js v0.x (TS predecessor) |   0.0145 |   70458 |   0.0235 |
| npm small (3 deps)   | napi v1.x (published)    |   0.0054 |  253263 |   0.0381 |
| npm small (3 deps)   | napi local (unpublished) |   0.0039 |  259055 |   0.0056 |
| npm large (500 deps) | js v0.x (TS predecessor) |   2.1331 |     469 |   2.1650 |
| npm large (500 deps) | napi v1.x (published)    |   0.3975 |    2529 |   0.4383 |
| npm large (500 deps) | napi local (unpublished) |   0.3885 |    2577 |   0.4049 |

## Rust vs JS speedup

| Fixture              |   JS v0.x | napi v1.x | Speedup |
| :------------------- | --------: | --------: | ------: |
| npm small (3 deps)   | 0.0145 ms | 0.0054 ms |   ~2.7x |
| npm large (500 deps) | 2.1331 ms | 0.3975 ms |   ~5.4x |
