import { readFile, readdir, stat } from 'node:fs/promises';
import { join } from 'node:path';

const NOT_FOUND = -1;
const PERCENTAGE_MULTIPLIER = 100;

type Estimate = {
  point_estimate: number;
  standard_error: number;
  confidence_interval: { lower_bound: number; upper_bound: number };
};

type Estimates = {
  mean: Estimate;
  median: Estimate;
  median_abs_dev: Estimate;
  slope?: Estimate;
  std_dev: Estimate;
};

export type BenchmarkResult = {
  name: string;
  baseNanoseconds: number;
  prNanoseconds: number;
  diffPercentage: number;
};

export type CrossComparison = {
  metric: string;
  ownName: string;
  ownNanoseconds: number;
  referenceName: string;
  referenceNanoseconds: number;
  speedup: number;
};

const directoryExists = async (path: string): Promise<boolean> => {
  try {
    const stats = await stat(path);
    return stats.isDirectory();
  } catch {
    return false;
  }
};

const fileExists = async (path: string): Promise<boolean> => {
  try {
    const stats = await stat(path);
    return stats.isFile();
  } catch {
    return false;
  }
};

/**
 * Recursively find all benchmark directories that contain both baselines.
 * Criterion stores results as: <criterion-dir>/<bench-id>/<baseline>/estimates.json
 * The bench-id may contain nested directories (e.g., "group/function").
 */
const findBenchmarkDirectories = async (
  criterionDirectory: string,
  baseBaseline: string,
  prBaseline: string,
): Promise<{ name: string; directory: string }[]> => {
  const results: { name: string; directory: string }[] = [];

  const walk = async (directory: string, prefix: string): Promise<void> => {
    const entries = await readdir(directory, { withFileTypes: true });
    const subdirectories = entries.filter(entry => entry.isDirectory());

    const hasBase = await directoryExists(join(directory, baseBaseline));
    const hasPr = await directoryExists(join(directory, prBaseline));

    if (hasBase && hasPr) {
      const baseEstimatesPath = join(directory, baseBaseline, 'estimates.json');
      const prEstimatesPath = join(directory, prBaseline, 'estimates.json');

      if ((await fileExists(baseEstimatesPath)) && (await fileExists(prEstimatesPath))) {
        results.push({ directory, name: prefix || directory });
        return;
      }
    }

    for (const subdirectory of subdirectories) {
      if (subdirectory.name === baseBaseline || subdirectory.name === prBaseline) {
        continue;
      }
      const subdirectoryPath = join(directory, subdirectory.name);
      const subdirectoryPrefix = prefix ? `${prefix}/${subdirectory.name}` : subdirectory.name;
      await walk(subdirectoryPath, subdirectoryPrefix);
    }
  };

  await walk(criterionDirectory, '');
  return results;
};

export const compareBenchmarks = async (
  criterionDirectory: string,
  baseBaseline: string,
  prBaseline: string,
): Promise<BenchmarkResult[]> => {
  if (!(await directoryExists(criterionDirectory))) {
    return [];
  }

  const benchmarkDirectories = await findBenchmarkDirectories(criterionDirectory, baseBaseline, prBaseline);
  const results: BenchmarkResult[] = [];

  for (const { name, directory } of benchmarkDirectories) {
    const baseFilePath = join(directory, baseBaseline, 'estimates.json');
    const prFilePath = join(directory, prBaseline, 'estimates.json');

    const baseEstimates: Estimates = JSON.parse(await readFile(baseFilePath, 'utf-8'));
    const prEstimates: Estimates = JSON.parse(await readFile(prFilePath, 'utf-8'));

    const baseNanoseconds = baseEstimates.mean.point_estimate;
    const prNanoseconds = prEstimates.mean.point_estimate;
    const diffPercentage =
      baseNanoseconds === 0 ? 0 : ((prNanoseconds - baseNanoseconds) / baseNanoseconds) * PERCENTAGE_MULTIPLIER;

    results.push({ baseNanoseconds, diffPercentage, name, prNanoseconds });
  }

  return results.toSorted((a, b) => a.name.localeCompare(b.name));
};

/**
 * Build cross-comparisons between own benchmarks and reference benchmarks.
 * Matches by shared suffix after the prefix separator (e.g. "riri: parse range"
 * pairs with "nodejs-semver: parse range" on the "parse range" suffix).
 */
export const buildCrossComparisons = (results: BenchmarkResult[], referencePrefixes: string[]): CrossComparison[] => {
  const SEPARATOR = '_ ';
  const isReference = (name: string): boolean => referencePrefixes.some(prefix => name.startsWith(prefix));

  const ownBenchmarks = results.filter(result => !isReference(result.name));
  const referenceBenchmarks = results.filter(result => isReference(result.name));

  const referenceByMetric = new Map<string, BenchmarkResult>();
  for (const ref of referenceBenchmarks) {
    const separatorIndex = ref.name.indexOf(SEPARATOR);
    if (separatorIndex !== NOT_FOUND) {
      const metric = ref.name.slice(separatorIndex + SEPARATOR.length);
      referenceByMetric.set(metric, ref);
    }
  }

  const comparisons: CrossComparison[] = [];
  for (const own of ownBenchmarks) {
    const separatorIndex = own.name.indexOf(SEPARATOR);
    if (separatorIndex === NOT_FOUND) {
      continue;
    }
    const metric = own.name.slice(separatorIndex + SEPARATOR.length);
    const ref = referenceByMetric.get(metric);
    if (!ref) {
      continue;
    }

    const ownPrefix = own.name.slice(0, own.name.indexOf(SEPARATOR));
    const refPrefix = ref.name.slice(0, ref.name.indexOf(SEPARATOR));
    const speedup = own.prNanoseconds === 0 ? 0 : ref.prNanoseconds / own.prNanoseconds;
    comparisons.push({
      metric,
      ownName: ownPrefix,
      ownNanoseconds: own.prNanoseconds,
      referenceName: refPrefix,
      referenceNanoseconds: ref.prNanoseconds,
      speedup,
    });
  }

  return comparisons.toSorted((a, b) => a.metric.localeCompare(b.metric));
};
