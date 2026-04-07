import { readdir, readFile } from "node:fs/promises";
import { join } from "node:path";

interface Estimate {
  point_estimate: number;
  standard_error: number;
  confidence_interval: { lower_bound: number; upper_bound: number };
}

interface Estimates {
  mean: Estimate;
  median: Estimate;
  median_abs_dev: Estimate;
  slope?: Estimate;
  std_dev: Estimate;
}

export interface BenchmarkResult {
  name: string;
  baseNs: number;
  prNs: number;
  diffPct: number;
}

async function dirExists(path: string): Promise<boolean> {
  try {
    const { stat } = await import("node:fs/promises");
    const s = await stat(path);
    return s.isDirectory();
  } catch {
    return false;
  }
}

async function fileExists(path: string): Promise<boolean> {
  try {
    const { stat } = await import("node:fs/promises");
    const s = await stat(path);
    return s.isFile();
  } catch {
    return false;
  }
}

/**
 * Recursively find all benchmark directories that contain both baselines.
 * Criterion stores results as: <criterion-dir>/<bench-id>/<baseline>/estimates.json
 * The bench-id may contain nested directories (e.g., "group/function").
 */
async function findBenchmarkDirs(
  criterionDir: string,
  baseBaseline: string,
  prBaseline: string,
): Promise<{ name: string; dir: string }[]> {
  const results: { name: string; dir: string }[] = [];

  async function walk(dir: string, prefix: string): Promise<void> {
    const entries = await readdir(dir, { withFileTypes: true });
    const subdirs = entries.filter((e) => e.isDirectory());

    const hasBase = await dirExists(join(dir, baseBaseline));
    const hasPr = await dirExists(join(dir, prBaseline));

    if (hasBase && hasPr) {
      const baseEstimates = join(dir, baseBaseline, "estimates.json");
      const prEstimates = join(dir, prBaseline, "estimates.json");

      if ((await fileExists(baseEstimates)) && (await fileExists(prEstimates))) {
        results.push({ name: prefix || dir, dir });
        return;
      }
    }

    for (const sub of subdirs) {
      if (sub.name === baseBaseline || sub.name === prBaseline) continue;
      const subPath = join(dir, sub.name);
      const subPrefix = prefix ? `${prefix}/${sub.name}` : sub.name;
      await walk(subPath, subPrefix);
    }
  }

  await walk(criterionDir, "");
  return results;
}

export async function compareBenchmarks(
  criterionDir: string,
  baseBaseline: string,
  prBaseline: string,
): Promise<BenchmarkResult[]> {
  if (!(await dirExists(criterionDir))) {
    return [];
  }

  const benchDirs = await findBenchmarkDirs(criterionDir, baseBaseline, prBaseline);
  const results: BenchmarkResult[] = [];

  for (const { name, dir } of benchDirs) {
    const baseFile = join(dir, baseBaseline, "estimates.json");
    const prFile = join(dir, prBaseline, "estimates.json");

    const baseEstimates: Estimates = JSON.parse(await readFile(baseFile, "utf-8"));
    const prEstimates: Estimates = JSON.parse(await readFile(prFile, "utf-8"));

    const baseNs = baseEstimates.mean.point_estimate;
    const prNs = prEstimates.mean.point_estimate;
    const diffPct = baseNs === 0 ? 0 : ((prNs - baseNs) / baseNs) * 100;

    results.push({ name, baseNs, prNs, diffPct });
  }

  return results.sort((a, b) => a.name.localeCompare(b.name));
}
