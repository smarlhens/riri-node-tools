# nodejs-semver v4.2.0 → v5.0.0: deep performance analysis

What changed, why it is faster, and what it means for this repo.

Source compared: crates.io `nodejs-semver-4.2.0` vs `nodejs-semver-5.0.0`
(upstream `github.com/cijiugechu/nodejs-semver`, 5.0.0 released 2026-06-20).

## TL;DR

v5 keeps the existing `winnow` combinator parser as a **correctness fallback** and
adds in front of it hand-written **byte-level fast paths** plus a **compact data
layout**. Common inputs (plain versions, `^`/`~`/comparators/wildcards/hyphen/`||`)
never touch `winnow` and allocate little or nothing. Net effect from upstream's own
`cargo bench --bench parser` (same machine, same suite):

| benchmark                                |     4.2.0 |     5.0.0 |    speedup |
| ---------------------------------------- | --------: | --------: | ---------: |
| `Version::parse("1.2.3")`                |  60.52 ns |   4.86 ns | **12.46x** |
| `Version::parse("1.2.3-rc.4+build.7")`   | 109.02 ns |  65.00 ns |      1.68x |
| `Range::parse("1.2.3")`                  | 333.84 ns |  35.91 ns |  **9.30x** |
| `Range::parse(">=1.2.3 <2.0.0")`         | 464.21 ns | 103.39 ns |      4.49x |
| `Range::parse(">=18 <20 \|\| >=22")`     | 584.19 ns | 261.19 ns |      2.24x |
| `Range::parse("1.x.x")`                  | 314.54 ns |  58.80 ns |      5.35x |
| parse + satisfy exact range              | 386.88 ns |  44.45 ns |      8.70x |
| parse + satisfy wildcard range           | 365.71 ns |  65.97 ns |      5.54x |
| filter version strings (current API)     | 774.59 ns | 221.29 ns |      3.50x |
| pkg-manager corpus: resolved versions    |   7.97 µs | 914.05 ns |  **8.71x** |
| pkg-manager corpus: range parse attempts |  40.91 µs |   7.39 µs |      5.54x |
| pkg-manager corpus: version-then-range   |  21.61 µs |   5.29 µs |      4.09x |

Grouped geomeans: version_parse **4.36x**, range_parse **4.20x**,
range_parse_fallback **3.97x**, parse_and_satisfies **3.72x**, pkg-manager corpus **5.01x**.

⚠️ Caveat: already-parsed `Range::satisfies` microbenchmarks (no parsing) are
**mixed — 0.94x geomean**, i.e. slightly _slower_. The win is entirely in parsing
and allocation, not in matching.

## File-level diff

| file                  | 4.2.0 | 5.0.0         | note                                          |
| --------------------- | ----- | ------------- | --------------------------------------------- |
| `src/lib.rs`          | 1552  | 2234          | Version layout rewrite, methods, `into_parts` |
| `src/range.rs`        | 2636  | 2723          | `Range` = SmallVec, fast-path dispatch        |
| `src/version_fast.rs` | —     | **148 (new)** | hand-rolled `Version::parse` fast path        |
| `src/range/fast.rs`   | —     | **888 (new)** | hand-rolled `Range::parse` fast paths + SWAR  |

`Cargo.toml`: edition `2021→2024`, MSRV `1.70.0→1.85.0`, added dependency
`smallvec = "1.15"`.

## The six optimizations

### 1. Fast-path byte parsers tried before `winnow`

Both entry points now try a hand-written byte scanner first and only fall through
to the general `winnow` combinator parser when it returns `None`.

`Version::parse` (`lib.rs`):

```rust
if input.len() > MAX_LENGTH { return Err(...); }
if let Some(version) = version_fast::parse(input) { return Ok(version); }   // fast
match version.parse_next(&mut input) { ... }                                // winnow fallback
```

`Range::parse` (`range.rs`) — a three-tier ladder, cheapest first:

```rust
if let Some(range) = fast::parse(input) { return Ok(range); }          // 1. structured fast path
if should_skip_range_fallback(input) { return Err(...); }              //    cheap reject
if let Some(range) = fast::parse_garbage(input) { return Ok(range); }  // 2. simple "garbage" path
match range_set.parse_next(&mut input) { ... }                         // 3. winnow fallback
```

`version_fast::parse` is a linear single-pass byte walk: optional `v`/`V`, optional
leading whitespace, then `core_number '.' core_number '.' core_number`, then a match
on the next byte to branch into `+build`, `-prerelease`, loose alnum suffix, or end.
Numbers parse with overflow-checked `checked_mul(10).checked_add(..)` and an explicit
`> MAX_SAFE_INTEGER` guard — same validity rules as the slow path, none of the
combinator/error-machinery overhead.

`range::fast::parse` dispatches on the first byte: `^`→caret, `~`→tilde,
`>`/`<`/`=`→comparator set, exact `x.y.z`, `x`/`X`/`*`→wildcard, digit/`v`→hyphen or
wildcard. Each builds `BoundSet`s directly. This is where the 9.30x on `Range::parse("1.2.3")`
comes from: the exact-version branch skips comparator parsing entirely.

`winnow` is retained verbatim, so weird/rare inputs still parse correctly — the fast
paths are pure additive shortcuts, returning `None` (never a wrong answer) on anything
they do not fully recognize.

### 2. SWAR byte scanning for short inputs (`ShortWord`)

Picking the right fast sub-path needs cheap "does this contain `+` / `-` / `||`"
probes. For inputs ≤ 16 bytes (`SHORT_SCAN_MAX`), v5 packs the string into one
`u128` and searches all 16 lanes branchlessly:

```rust
fn matching_byte_lanes(word: u128, needle: u8) -> u128 {
    let zero_bytes = word ^ repeated_byte(needle);          // matches → 0x00 lanes
    zero_bytes.wrapping_sub(LOW_BITS) & !zero_bytes & HIGH_BITS  // classic zero-byte test
}
```

- `contains_byte(needle)` = any lane matches within the active length.
- `contains_repeated_pair(b'|')` = `mask & (mask >> 8)` → detects `||` adjacency in
  a couple of ALU ops, replacing `str::contains("||")`.

Inputs > 16 bytes fall back to `slice::contains` / `str::contains`. Almost every real
npm version/range string is ≤ 16 bytes, so the SWAR path dominates. `active_byte_mask`
masks off the zero padding so trailing `\0`s never produce false matches (covered by tests).

### 3. Compact `Version` — no heap alloc for the common case

```rust
// v4.2.0  (~72 bytes, two Vec headers, two heap allocs even for "1.2.3")
pub struct Version { pub major:u64, pub minor:u64, pub patch:u64,
                     pub build:Vec<Identifier>, pub pre_release:Vec<Identifier> }

// v5.0.0  (32 bytes; meta = None for plain versions → ZERO heap alloc)
pub struct Version { major:u64, minor:u64, patch:u64, meta:Option<Box<VersionMeta>> }
struct VersionMeta { build:Identifiers, pre_release:Identifiers }
```

`new_empty` / `new_with_identifiers` short-circuit to `meta: None` whenever both
prerelease and build are empty. Prerelease/build metadata is boxed and allocated
**only when actually present**. This is the single biggest contributor to the
12.46x on `"1.2.3"` — the v4 version did two heap allocations for empty Vecs that
v5 elides entirely.

### 4. `Identifiers` inline-small enum

```rust
enum Identifiers { Empty, One(Identifier), Two([Identifier;2]), Many(Vec<Identifier>) }
```

Replaces `Vec<Identifier>`. The overwhelming majority of prerelease/build tags have
0, 1, or 2 identifiers (`-rc.4`, `+build.7`, `-alpha.1`) and now live inline with no
heap allocation; only 3+ spill to `Many(Vec)`. `push` grows `Empty → One → Two → Many`
in place. This is why `"1.2.3-rc.4+build.7"` still improves (1.68x) despite needing metadata.

### 5. `Range` = `SmallVec<[BoundSet; 1]>`

```rust
pub struct Range(SmallVec<[BoundSet; 1]>);   // was Vec<BoundSet>
```

The common range is a single comparator set (`^1`, `>=1 <2`, `1.x`, exact). It now
stores its one `BoundSet` inline on the stack — no heap allocation. Only disjunctions
(`||`, 2+ sets) spill to the heap. `from_bound_set` builds the inline-1 case directly;
disjunction paths (`parse_or`, `parse_garbage`) use `from_bound_sets`.

### 6. `VersionParts` + `into_parts()`, and privatized fields

```rust
pub fn into_parts(self) -> VersionParts   // moves prerelease/build Vecs out, no clone
```

Lets callers extract owned components without cloning the identifier vectors. The
enabling change is that `Version`'s fields are now **private** (the breaking change):
the internal `Option<Box<VersionMeta>>` / `Identifiers` representation can keep being
tuned without breaking the public API again. Read access moved to methods:
`major()`, `minor()`, `patch()`, `pre_release() -> &[Identifier]`, `build() -> &[Identifier]`.

### Supporting: fallback gating

Beyond the fast paths, v5 avoids `winnow` even on near-misses:

- `should_skip_range_fallback` cheaply rejects single unparseable tokens before the
  expensive parser runs.
- `fast::parse_garbage` handles simple whitespace-separated "garbage" ranges
  (`"1.2.3 foo"` → `1.2.3`) without `winnow`; bails (`None`) on anything with
  operators/prerelease so correctness stays with the fallback.
- `parse_or_if_present` only runs the `||`-splitting path when `contains_or_fast`
  (SWAR) confirms a `||` is actually present.

## Impact on this repo (`riri-node-tools`)

`nodejs-semver` is **not a runtime dependency of any shipped crate**. It appears only
in `crates/riri-semver-range` as:

- **test oracle** — `tests/cross_validate_nodejs_semver.rs`, `tests/proptest_invariants.rs`
  (`satisfies_matches_nodejs_semver`): we assert our own `riri-semver-range` matches
  npm semantics by diffing against `nodejs_semver`.
- **benchmark baseline** — `benches/range_parsing.rs`, `benches/range_satisfies.rs`:
  `riri` vs `nodejs_semver` head-to-head.

Both use only `Range::parse` / `Version::parse` — public, unchanged signatures. The
v5 breaking changes (private fields, edition 2024, MSRV 1.85) touch none of our code,
which is why the bump (`ba4033b`) was Cargo.toml + Cargo.lock only.

Two consequences worth noting:

1. **Benchmark baseline got 4–9x faster on parsing.** Any "we parse faster than
   nodejs-semver" framing in `BENCHMARKS.md` / `range_parsing.rs` should be re-measured
   against 5.0.0 — the gap narrowed substantially or may have flipped.
2. **`satisfies` baseline barely moved (geomean 0.94x).** `range_satisfies.rs` parses
   once then matches in a loop, so that comparison is largely unaffected by the v5 work.
   Our MSRV must also be ≥ 1.85 to keep compiling the dev-dependency (workspace is
   already edition 2024, so fine).

## Feature comparison: `riri-semver-range` vs `nodejs-semver`

They are **different kinds of tool**. `nodejs-semver` is a general-purpose, npm-compatible
semver library (parse + match + compare). `riri-semver-range` is a domain-specific
**npm `engines`-range analyzer and humanizer** that delegates version _representation_ to
dtolnay `semver` and adds range-level _reasoning_ on top.

### Only in `riri-semver-range`

- `ParsedRange::humanize` / `humanize_with(VersionPrecision)` — render a range back to a
  canonical human string (`^16.10.0`, `>=14.0.0 <18.0.0`); precision trimming of trailing `.0`.
  This is the crate's reason to exist.
- `restrictive_range(r1, r2)` — intersect two ranges into the tighter one.
- `is_subset_of` / `intersects` — range-vs-range containment & overlap reasoning.
- `split_by_major` — split a cross-major interval into one part per major.
- `RangePart::is_caret` — recognize a normalized interval that came from a caret.
- Explicit operator-interval model (`min`/`min_op`/`max`/`max_op`) — purpose-built for
  humanizing and interval math, vs nodejs-semver's `BoundSet`/`Predicate` form.
- `satisfies` with node-`engines`-focused prerelease filtering.

### Only in `nodejs-semver` (and mostly not needed here)

- Its own full `Version` type with prerelease/build identifier ordering (we use dtolnay
  `semver::Version` for this — already correct and fast).
- `max_satisfying` / `min_satisfying`, version `diff`, serde for `Version`/`Range`.
- Broad npm compatibility for _weird_ inputs: loose suffixes (`1.2.3beta`), simple "garbage"
  ranges (`1.2.3 foo`), and the full hyphen/prerelease/disjunction edge cases (explicitly
  expanded in v5). Our parser handles the common engines subset; divergence on odd inputs is
  exactly what `tests/cross_validate_nodejs_semver.rs` and `proptest_invariants.rs` guard.

So a fair summary: we are not missing general features by accident — we deliberately cover a
narrower, opinionated slice and add analysis nodejs-semver doesn't have.

## Where our time actually goes

Consumers (`riri-nce`, `riri-ncd`, `riri-napi-nce`) parse `engines` ranges across dependency
trees, so `ParsedRange::parse` is on a hot path. Crucially, **dtolnay `semver::Version` is
already compact and alloc-free for plain versions** — so unlike nodejs-semver v5, we do _not_
need to rework a version type. Our waste is elsewhere: we decompose a version into `u64`s and
then **re-serialize it to a `String` and re-parse it**, and we always heap-allocate the parts
vector. The v5 techniques map onto these directly.

## Perf opportunities, ranked

### 1. Kill the String round-trip in `parse_partial_version` — biggest win

`parse.rs:539-578` splits the input, validates each component with `parse_u64`, then does
`format!("{major}.{minor}.{patch}")` and calls `semver::Version::parse(&full)` — re-parsing
numbers it already has. For the no-prerelease case (the overwhelming majority of engine
ranges) build the version directly:

```rust
// after parsing major/minor/patch as u64 and confirming no `-pre`:
Version::new(major, minor, patch)          // alloc-free, no string, no re-parse
// only when a prerelease is present, fall back to the format!+Version::parse path
```

This is the exact analog of nodejs-semver's `new_empty` fast path. Eliminates a `Vec<&str>`,
a `String`, and a full semver re-parse per partial version — and partial versions are built
for nearly every comparator (`^`, `~`, `>=`, x-ranges, bare).

### 2. `ParsedRange.parts: SmallVec<[RangePart; 1]>`

`parse.rs:6` — most engine ranges are a single comparator set (no `||`), yet `Vec<RangePart>`
always heap-allocates. Switching to `SmallVec<[RangePart; 1]>` keeps the common single-part
range entirely on the stack. Direct analog of v5's `Range(SmallVec<[BoundSet; 1]>)`. Cost: add
`smallvec` as a real dependency (already in the tree via dev-deps) and update the few `.parts`
constructors/iterators. `RangePart` is ~120 bytes, so inline-1 is fine; don't go to inline-2.

### 3. Tokenize without `Vec<String>` + `format!`

`tokenize_comparator_set` (`parse.rs:227-241`) builds owned `String`s, merging a bare operator
with the next token via `format!("{}{}", op, ver)`. Rework so the merge passes the operator and
the version slice **separately** into `parse_single_comparator` (e.g. a
`parse_comparator(op: Option<&str>, ver: &str)`), so tokens stay `&str`. At minimum collect into
`SmallVec<[&str; 2]>` of borrowed slices. Removes one `String` per token on the parse hot path.
This is our version of the "scan bytes, don't allocate" fast path.

### 4. Single-pass caret/tilde parsing

`parse_caret`/`parse_tilde` (`parse.rs:360-415`) call `version_component_count(input)` — which
scans the string (`split('+')`, `find('-')`, `split('.')`, `take_while`, `count`) — and then
`parse_partial_version(input)` re-scans the same string. Two full passes for one short token.
Have the partial-version parser return `(Version, component_count)` in a single walk.

### 5. Single-pass `is_x_range`

`is_x_range` (`parse.rs:256-261`) does up to four scans: `contains('x') || contains('X') ||
contains('*') || split('.').count() < 3`. Called for every token. Fold into one byte walk that
records whether any of `x`/`X`/`*` appears and counts `.` separators.

### 6. (Secondary) trim clone/`Vec` churn in interval math

`restrictive_range` clones whole `ParsedRange`s up to four times before doing work
(`intersection.rs:13-31`); `split_by_major` returns `vec![part.clone()]` even on the no-split
path (`helpers.rs:11,24`); `intersect_parts` clones versions on every bound pick. Colder than
parse, but `restrictive_range` runs pairwise over split parts, so for multi-major ranges these
add up. Consider `SmallVec` returns and borrowing where the part is unchanged.

### Not worth it

Reworking `RangePart` to drop the two full `semver::Version`s for a packed `(u64,u64,u64)` +
optional prerelease would shrink the struct, but it churns the public API (`pub min: Version`)
and dtolnay `Version` is already cheap to compare. Skip unless profiling says otherwise.

### Suggested order

Do **1 → 2 → 3** first (all parse-path, low risk, directly mirror v5, guarded by the existing
cross-validate + proptest suites), re-run `benches/range_parsing.rs` against nodejs-semver
5.0.0 to quantify, then decide on 4–6 from the numbers.

## Implemented on this branch (1–7)

All seven parse-path optimizations are implemented and measured. Prerelease handling is
**unchanged**: the fast path in #1 fires only when the token has no `-pre`; any prerelease
token falls through to the original `format!` + `Version::parse` route, so identifiers are
still parsed and validated.

- **#1** `parse_partial_version` (`parse.rs`): no-prerelease tokens now build
  `Version::new(major, minor, patch)` directly via a `parse_core_components` helper —
  no intermediate `String`, no full re-parse, no `Vec<&str>`.
- **#2** `ParsedRange.parts` is now `SmallVec<[RangePart; 1]>` (type alias `Parts`). Single-part
  ranges (no `||`) stay on the stack. Consumers were unaffected except three `ParsedRange { .. }`
  construction sites in `riri-nce/src/policy.rs`, which gained a `.into()` (Vec → SmallVec, no new
  dependency — `From<Vec>` resolves through the public field type).
- **#3** `tokenize_comparator_set` (allocated a `Vec<String>` and `format!`-merged bare operators)
  is replaced by `comparator_tokens`, which yields borrowed `&str` slices of the original input
  (`SmallVec<[&str; 2]>`); a merged operator+version token is a single slice spanning both words,
  with the embedded whitespace trimmed downstream by `strip_v`.
- **#4** caret/tilde no longer scan the token twice. `version_component_count` is gone; the new
  `parse_partial_with_count` returns the version **and** the component count from one scan, which
  `parse_caret`/`parse_tilde` consume — relevant because `^` is the most common range form.
- **#5** `is_x_range` was up to four scans (`contains('x')` ×3 + `split('.').count()`); now a single
  byte pass that early-returns on a wildcard byte and counts dots otherwise.
- **#6** `split_by_major` returned `vec![part.clone()]` on the common no-split path (a heap alloc per
  part); it now returns `SmallVec<[RangePart; 1]>` (`SplitParts`), keeping the single part inline.
  Benefits the humanize / intersection / policy paths. `policy.rs` adapted with one `.into_vec()`.
- **#7 (the big lever)** a byte-level `fast_comparator` fast path on `parse_single_comparator`,
  modelled on v5's `range::fast::parse`. It handles an optional operator followed by a
  fully-specified `major.minor.patch[-pre]` version (`^1.2.3`, `>=16.0.0`, `~1.2.3`, `1.2.3`,
  `=1.2.3-alpha`, `> 1.0.0`) in a single byte walk — one operator dispatch, inline digit
  accumulation, direct `RangePart` construction — collapsing all the per-token `split`/`strip_prefix`/
  re-scan work. Wildcards/x-ranges, partials, build metadata and hyphen ranges return `None` and fall
  through to the unchanged string parser, so it is a pure shortcut with no behavioural change. A
  companion guard skips the full `" - "` hyphen scan for operator-led sets.

### Measured (`cargo bench --bench range_parsing`, same machine, 8-range corpus)

|                      | before (main) | after #1–6 |    after #1–7 | nodejs-semver 5.0.0 |
| -------------------- | ------------: | ---------: | ------------: | ------------------: |
| parse range (corpus) |      2.149 µs |   1.071 µs | **581 ns** |              829 ns |

- **3.70x faster** than before (~73% less parse time).
- **Now faster than nodejs-semver 5.0.0**: 581 ns vs 829 ns — riri is **1.43x faster** (was 2.58x
  slower before this work). 7 of the 8 corpus entries hit the fast path; the wildcard `1.2.x` and the
  empty/`*` cases use the fallback.

Verification: all 22 test suites pass — including `cross_validate_nodejs_semver` (non-prerelease +
a prerelease corpus, 432 comparisons, 0 mismatches) and `proptest_invariants` (a 2000-case prerelease
fuzz vs nodejs-semver, plus the existing non-prerelease fuzz) — and clippy is clean under the
workspace's `pedantic = deny`. The fast path is validated by equivalence: any divergence from the
slow path would surface as a cross-validation or fuzz mismatch.

Remaining headroom is small and not worth the risk: the fallback still handles x-ranges/partials/
hyphens with the string parser, and a SWAR short-string probe (v5 §2) would help only very long
inputs, which engine ranges never are.
