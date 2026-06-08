# nce benchmarks

## Microbenchmarks — Rust (criterion)

| Benchmark                         |     Time |
| :-------------------------------- | -------: |
| check*engines* 500 deps           |  65.0 µs |
| check*engines* 7 deps (or-ranges) |   6.8 µs |
| parse npm lockfile\_ 500 deps     | 231.2 µs |

## JS vs napi (tinybench)

| Fixture          | Variant                  | avg (ms) | ops/sec | p99 (ms) |
| :--------------- | :----------------------- | -------: | ------: | -------: |
| small (7 deps)   | js v0.x (TS predecessor) |   0.1262 |    9247 |   0.4216 |
| small (7 deps)   | napi v1.x (published)    |   0.0167 |   62143 |   0.0389 |
| small (7 deps)   | napi local (unpublished) |   0.0169 |   62290 |   0.0453 |
| large (500 deps) | js v0.x (TS predecessor) |   1.3543 |     742 |   1.4799 |
| large (500 deps) | napi v1.x (published)    |   0.4264 |    2371 |   0.4857 |
| large (500 deps) | napi local (unpublished) |   0.3807 |    2635 |   0.4032 |

## Rust vs JS speedup

| Fixture          |   JS v0.x | napi v1.x | Speedup |
| :--------------- | --------: | --------: | ------: |
| small (7 deps)   | 0.1262 ms | 0.0167 ms |   ~7.5x |
| large (500 deps) | 1.3543 ms | 0.4264 ms |   ~3.2x |
