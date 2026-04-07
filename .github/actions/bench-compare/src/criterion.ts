import { readFile, readdir, stat } from 'node:fs/promises';
import { join } from 'node:path';

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
