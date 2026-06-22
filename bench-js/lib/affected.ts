// bench-js/lib/affected.ts
//
// Resolve which benchable crates a diff affects, so pr-bench can skip the rest.
// A benchable crate is affected when it (or any of its transitive workspace
// path-dependencies) changed. Anything that is not a `crates/<pkg>/**` file
// (Cargo.lock, root Cargo.toml, rustfmt.toml, fixtures, scripts, workflows)
// touches the whole build or every bench fixture, so it forces all crates.

export interface CargoDependency {
  name: string;
}
export interface CargoPackage {
  name: string;
  manifest_path: string;
  dependencies: CargoDependency[];
}
export interface CargoMetadata {
  packages: CargoPackage[];
}

// Crates that own criterion benches. Everything resolved is a subset of these.
const BENCHABLE = ['riri-nce', 'riri-ncd', 'riri-npd', 'riri-semver-range'];

const shortName = (pkg: string): string => pkg.replace(/^riri-/, '');

// `crates/<dir>` for the file, or undefined when it lives elsewhere.
const crateDirOf = (file: string): string | undefined => {
  if (!file.startsWith('crates/')) return undefined;
  const segments = file.split('/');
  return segments.length >= 2 ? `crates/${segments[1]}` : undefined;
};

// Map every `crates/<dir>` to its package name via the manifest path.
const dirToPackage = (meta: CargoMetadata): Map<string, string> => {
  const map = new Map<string, string>();
  for (const pkg of meta.packages) {
    const dir = pkg.manifest_path.replace(/\/Cargo\.toml$/, '');
    map.set(dir, pkg.name);
  }
  return map;
};

// Adjacency of each package to its workspace path-dependencies only.
const workspaceAdjacency = (meta: CargoMetadata): Map<string, string[]> => {
  const members = new Set(meta.packages.map(pkg => pkg.name));
  const adjacency = new Map<string, string[]>();
  for (const pkg of meta.packages) {
    const deps = pkg.dependencies.map(dep => dep.name).filter(name => members.has(name));
    adjacency.set(pkg.name, [...new Set(deps)]);
  }
  return adjacency;
};

// Package + all its transitive workspace dependencies.
const closure = (start: string, adjacency: Map<string, string[]>): Set<string> => {
  const seen = new Set([start]);
  const frontier = [start];
  while (frontier.length > 0) {
    const current = frontier.pop() as string;
    for (const next of adjacency.get(current) ?? []) {
      if (!seen.has(next)) {
        seen.add(next);
        frontier.push(next);
      }
    }
  }
  return seen;
};

export const affectedBenchableCrates = (changedFiles: string[], meta: CargoMetadata): string[] => {
  const lookup = dirToPackage(meta);

  let forceAll = false;
  const changedPackages = new Set<string>();
  for (const file of changedFiles) {
    const dir = crateDirOf(file);
    if (dir === undefined) {
      forceAll = true;
      continue;
    }
    // Manifest paths are absolute; match on the trailing `crates/<dir>`.
    const pkg = [...lookup].find(([manifestDir]) => manifestDir === dir || manifestDir.endsWith(`/${dir}`))?.[1];
    if (pkg === undefined) forceAll = true;
    else changedPackages.add(pkg);
  }

  if (forceAll) return BENCHABLE.map(shortName);

  const adjacency = workspaceAdjacency(meta);
  return BENCHABLE.filter(crate => [...closure(crate, adjacency)].some(dep => changedPackages.has(dep))).map(shortName);
};
