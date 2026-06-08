// bench-js/lib/criterion.ts
import { readFileSync, readdirSync, existsSync } from 'node:fs';
import { join } from 'node:path';

export interface Estimate {
  meanNs: number;
  stdDevNs: number;
}
export interface CriterionBench {
  name: string;
  estimatesPath: string;
}
export interface CrossEntry {
  name: string;
  ns: number;
}
export interface CrossComparison {
  metric: string;
  ownName: string;
  ownNs: number;
  referenceName: string;
  referenceNs: number;
  speedup: number;
}

export const readEstimates = (estimatesPath: string): Estimate => {
  const j = JSON.parse(readFileSync(estimatesPath, 'utf8'));
  return { meanNs: Math.round(j.mean.point_estimate), stdDevNs: Math.round(j.mean.standard_error) };
};

// Recursively find `<bench>/<baseline>/estimates.json`; name is the bench dir.
export const findCriterionBenches = (criterionDir: string, baseline: string): CriterionBench[] => {
  if (!existsSync(criterionDir)) return [];
  const out: CriterionBench[] = [];
  const walk = (dir: string): void => {
    for (const entry of readdirSync(dir, { withFileTypes: true })) {
      if (!entry.isDirectory()) continue;
      const child = join(dir, entry.name);
      const estimatesPath = join(child, baseline, 'estimates.json');
      if (existsSync(estimatesPath)) out.push({ name: entry.name, estimatesPath });
      else walk(child);
    }
  };
  walk(criterionDir);
  return out;
};

const SEPARATOR = '_ ';

// Pair own-vs-reference benches on the suffix after the "_ " separator.
export const crossCompare = (entries: CrossEntry[], referencePrefixes: string[]): CrossComparison[] => {
  const isReference = (name: string): boolean => referencePrefixes.some(prefix => name.startsWith(prefix));
  const referenceByMetric = new Map<string, CrossEntry>();
  for (const entry of entries.filter(e => isReference(e.name))) {
    const index = entry.name.indexOf(SEPARATOR);
    if (index !== -1) referenceByMetric.set(entry.name.slice(index + SEPARATOR.length), entry);
  }
  const out: CrossComparison[] = [];
  for (const own of entries.filter(e => !isReference(e.name))) {
    const index = own.name.indexOf(SEPARATOR);
    if (index === -1) continue;
    const metric = own.name.slice(index + SEPARATOR.length);
    const reference = referenceByMetric.get(metric);
    if (!reference) continue;
    out.push({
      metric,
      ownName: own.name.slice(0, index),
      ownNs: own.ns,
      referenceName: reference.name.slice(0, reference.name.indexOf(SEPARATOR)),
      referenceNs: reference.ns,
      speedup: own.ns === 0 ? 0 : reference.ns / own.ns,
    });
  }
  return out.toSorted((a, b) => a.metric.localeCompare(b.metric));
};

export interface CriterionRow {
  name: string;
  mean_ns: number;
  stddev_ns: number;
}

export const collectCriterion = (criterionDir: string): CriterionRow[] =>
  findCriterionBenches(criterionDir, 'new').map(({ name, estimatesPath }) => {
    const { meanNs, stdDevNs } = readEstimates(estimatesPath);
    return { name, mean_ns: meanNs, stddev_ns: stdDevNs };
  });
