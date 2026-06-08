import { crossCompare, findCriterionBenches, readEstimates } from '../../../../bench-js/lib/criterion.ts';

export type { CrossComparison } from '../../../../bench-js/lib/criterion.ts';

const PERCENTAGE_MULTIPLIER = 100;

export type BenchmarkResult = {
  name: string;
  baseNanoseconds: number;
  prNanoseconds: number;
  diffPercentage: number;
};

export const compareBenchmarks = async (
  criterionDirectory: string,
  baseBaseline: string,
  prBaseline: string,
): Promise<BenchmarkResult[]> => {
  const base = new Map(
    findCriterionBenches(criterionDirectory, baseBaseline).map(b => [b.name, readEstimates(b.estimatesPath).meanNs]),
  );
  const results: BenchmarkResult[] = [];
  for (const { name, estimatesPath } of findCriterionBenches(criterionDirectory, prBaseline)) {
    const prNanoseconds = readEstimates(estimatesPath).meanNs;
    const baseNanoseconds = base.get(name);
    if (baseNanoseconds === undefined) {
      continue;
    }
    const diffPercentage =
      baseNanoseconds === 0 ? 0 : ((prNanoseconds - baseNanoseconds) / baseNanoseconds) * PERCENTAGE_MULTIPLIER;
    results.push({ baseNanoseconds, diffPercentage, name, prNanoseconds });
  }
  return results.toSorted((a, b) => a.name.localeCompare(b.name));
};

export { crossCompare };
