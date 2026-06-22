// bench-js/lib/ci-bench.ts
//
// Pure helpers for the bench-run CI orchestrator: crate-set resolution, bench
// target discovery from `cargo metadata`, and parsers for `cargo test --list`,
// GNU `time -v`, and npm lockfiles. The I/O lives in ../ci-bench.ts.

export interface CargoTarget {
  name: string;
  kind: string[];
}
export interface CargoPackageTargets {
  name: string;
  targets: CargoTarget[];
}
export interface CargoMetadataTargets {
  packages: CargoPackageTargets[];
}

// Crates that own criterion benches; the binary crates are a subset.
export const BENCHABLE = ['nce', 'ncd', 'npd', 'semver-range'];
export const BINARIES = ['nce', 'npd', 'ncd'];

// Peak-RSS workloads: each binary measured against a shared 500-dep fixture.
// `ncd` needs a registry, so the caller synthesizes an offline one.
export interface RssTarget {
  binary: string;
  fixture: string;
  needsRegistry: boolean;
}
export const RSS_TARGETS: RssTarget[] = [
  { binary: 'nce', fixture: 'fixtures/npm-v3-500-deps', needsRegistry: false },
  { binary: 'npd', fixture: 'fixtures/npd-npm-v3-500-deps', needsRegistry: false },
  { binary: 'ncd', fixture: 'fixtures/npd-npm-v3-500-deps', needsRegistry: true },
];

export interface CrateSet {
  set: string[];
  buildPackages: string[];
}

// Empty request → every benchable crate. `buildPackages` are the cargo `-p`
// package names of the binary crates in the set.
export const resolveCrateSet = (requested: string): CrateSet => {
  const set = requested.trim().length > 0 ? requested.trim().split(/\s+/) : [...BENCHABLE];
  const buildPackages = set.filter(crate => BINARIES.includes(crate)).map(crate => `riri-${crate}`);
  return { set, buildPackages };
};

// Bench target names for the crate set, read from `cargo metadata` so only
// targets that exist at this ref are selected (no `--benches`, which would also
// run the lib/bin unittest harness).
export const benchTargets = (set: string[], meta: CargoMetadataTargets): string[] => {
  const packages = new Set(set.map(crate => `riri-${crate}`));
  return meta.packages
    .filter(pkg => packages.has(pkg.name))
    .flatMap(pkg => pkg.targets.filter(target => target.kind.includes('bench')).map(target => target.name))
    .toSorted((a, b) => a.localeCompare(b));
};

// Peak resident set size in KB from GNU `time -v` stderr; 0 when absent.
export const parseMaxRssKb = (timeOutput: string): number => {
  const match = timeOutput.match(/Maximum resident set size[^\d]*(\d+)/);
  return match ? Number(match[1]) : 0;
};

// Count of `cargo test -- --list` entries (lines ending in `: test`).
export const parseTestCount = (listOutput: string): number =>
  listOutput.split('\n').filter(line => line.endsWith(': test')).length;

// Locked package names (`node_modules/<name>`) from an npm lockfile, used to
// synthesize an offline registry of empty packuments for the ncd measurement.
export const packumentNames = (lockfileText: string): string[] => {
  const lockfile: { packages?: Record<string, unknown> } = JSON.parse(lockfileText);
  return Object.keys(lockfile.packages ?? {})
    .filter(key => key.startsWith('node_modules/'))
    .map(key => key.slice('node_modules/'.length));
};
