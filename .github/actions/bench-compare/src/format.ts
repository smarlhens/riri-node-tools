import { markdownTable } from 'markdown-table';

import type { BenchmarkResult, CrossComparison } from './criterion.js';

const NANOSECONDS_PER_SECOND = 1_000_000_000;
const NANOSECONDS_PER_MILLISECOND = 1_000_000;
const NANOSECONDS_PER_MICROSECOND = 1_000;
const BYTES_PER_KILOBYTE = 1_024;
const BYTES_PER_MEGABYTE = BYTES_PER_KILOBYTE * BYTES_PER_KILOBYTE;
const PERCENTAGE_MULTIPLIER = 100;
const DECIMAL_PLACES_SHORT = 1;
const DECIMAL_PLACES_LONG = 2;
const IMPROVEMENT_THRESHOLD = -1;

const bytesFormatter = new Intl.NumberFormat('en', {
  maximumFractionDigits: DECIMAL_PLACES_LONG,
  style: 'unit',
  unit: 'megabyte',
});
const kiloBytesFormatter = new Intl.NumberFormat('en', {
  maximumFractionDigits: DECIMAL_PLACES_SHORT,
  style: 'unit',
  unit: 'kilobyte',
});

const formatNanoseconds = (nanoseconds: number): string => {
  if (nanoseconds >= NANOSECONDS_PER_SECOND) {
    return `${(nanoseconds / NANOSECONDS_PER_SECOND).toFixed(DECIMAL_PLACES_LONG)} s`;
  }
  if (nanoseconds >= NANOSECONDS_PER_MILLISECOND) {
    return `${(nanoseconds / NANOSECONDS_PER_MILLISECOND).toFixed(DECIMAL_PLACES_LONG)} ms`;
  }
  if (nanoseconds >= NANOSECONDS_PER_MICROSECOND) {
    return `${(nanoseconds / NANOSECONDS_PER_MICROSECOND).toFixed(DECIMAL_PLACES_SHORT)} \u00B5s`;
  }
  return `${nanoseconds.toFixed(DECIMAL_PLACES_SHORT)} ns`;
};

const formatBytes = (bytes: number): string => {
  const absolute = Math.abs(bytes);
  if (absolute >= BYTES_PER_MEGABYTE) {
    return bytesFormatter.format(bytes / BYTES_PER_MEGABYTE);
  }
  if (absolute >= BYTES_PER_KILOBYTE) {
    return kiloBytesFormatter.format(bytes / BYTES_PER_KILOBYTE);
  }
  return `${bytes} B`;
};

const toPercentage = (numerator: number, denominator: number): number =>
  denominator === 0 ? 0 : (numerator / denominator) * PERCENTAGE_MULTIPLIER;

const formatPercentage = (percentage: number): string => {
  const sign = percentage > 0 ? '+' : '';
  return `${sign}${percentage.toFixed(DECIMAL_PLACES_LONG)}%`;
};

const regressionIcon = (percentage: number, threshold: number): string => {
  if (percentage > threshold) {
    return ':x:';
  }
  if (percentage > 0) {
    return ':warning:';
  }
  if (percentage < IMPROVEMENT_THRESHOLD) {
    return ':white_check_mark:';
  }
  return '';
};

export const formatBenchmarkTable = (results: BenchmarkResult[], threshold: number): string => {
  if (results.length === 0) {
    return '_No criterion benchmarks found._';
  }

  const rows = results.map(result => [
    result.name,
    formatNanoseconds(result.baseNanoseconds),
    formatNanoseconds(result.prNanoseconds),
    formatPercentage(result.diffPercentage),
    regressionIcon(result.diffPercentage, threshold),
  ]);

  return markdownTable([['Benchmark', 'Base', 'PR', '\u0394', ''], ...rows], { align: ['l', 'r', 'r', 'r', 'c'] });
};

export const formatBinarySizeTable = (prSizes: Record<string, number>, baseSizes: Record<string, number>): string => {
  const binaries = [...new Set([...Object.keys(prSizes), ...Object.keys(baseSizes)])].toSorted();

  if (binaries.length === 0) {
    return '_No binary size data._';
  }

  const rows = binaries.map(name => {
    const prSize = prSizes[name];
    const baseSize = baseSizes[name];

    if (prSize !== undefined && baseSize !== undefined) {
      const difference = prSize - baseSize;
      const percentage = toPercentage(difference, baseSize);
      return [
        `\`${name}\``,
        formatBytes(baseSize),
        formatBytes(prSize),
        formatBytes(difference),
        formatPercentage(percentage),
      ];
    }
    if (prSize !== undefined) {
      return [`\`${name}\``, '_new_', formatBytes(prSize), '-', '-'];
    }
    return [`\`${name}\``, formatBytes(baseSize ?? 0), '_removed_', '-', '-'];
  });

  return markdownTable([['Binary', 'Base', 'PR', 'Diff', '%'], ...rows], { align: ['l', 'r', 'r', 'r', 'r'] });
};

export const formatRssTable = (prRssKilobytes: number, baseRssKilobytes: number): string => {
  if (prRssKilobytes === 0 && baseRssKilobytes === 0) {
    return '_No memory data._';
  }

  const differenceKilobytes = prRssKilobytes - baseRssKilobytes;
  const percentage = toPercentage(differenceKilobytes, baseRssKilobytes);

  const rows = [
    [
      'Peak RSS',
      formatBytes(baseRssKilobytes * BYTES_PER_KILOBYTE),
      formatBytes(prRssKilobytes * BYTES_PER_KILOBYTE),
      formatBytes(differenceKilobytes * BYTES_PER_KILOBYTE),
      formatPercentage(percentage),
    ],
  ];

  return markdownTable([['Metric', 'Base', 'PR', 'Diff', '%'], ...rows], { align: ['l', 'r', 'r', 'r', 'r'] });
};

export const formatTestCountTable = (prCount: number, baseCount: number): string => {
  const difference = prCount - baseCount;
  let differenceString: string;
  if (difference > 0) {
    differenceString = `+${difference}`;
  } else if (difference === 0) {
    differenceString = '0';
  } else {
    differenceString = `${difference}`;
  }

  return markdownTable(
    [
      ['', 'Base', 'PR', 'Diff'],
      ['Test count', `${baseCount}`, `${prCount}`, differenceString],
    ],
    { align: ['l', 'r', 'r', 'r'] },
  );
};

export const formatCrossComparisonTable = (comparisons: CrossComparison[]): string => {
  if (comparisons.length === 0) {
    return '_No cross-comparison data._';
  }

  const firstOwn = comparisons[0].ownName;
  const firstRef = comparisons[0].referenceName;

  const rows = comparisons.map(comparison => [
    comparison.metric,
    formatNanoseconds(comparison.ownNanoseconds),
    formatNanoseconds(comparison.referenceNanoseconds),
    `${comparison.speedup.toFixed(DECIMAL_PLACES_LONG)}x`,
  ]);

  return markdownTable([['Metric', firstOwn, firstRef, 'Speedup'], ...rows], {
    align: ['l', 'r', 'r', 'r'],
  });
};

export const buildFullComment = (
  benchmarkTable: string,
  crossComparisonTable: string,
  sizeTable: string,
  rssTable: string,
  testTable: string,
  threshold: number,
  maxRegression: number,
): string => {
  const regressionStatus =
    maxRegression > threshold
      ? `**regression detected: ${maxRegression.toFixed(DECIMAL_PLACES_SHORT)}%**`
      : 'none detected';

  return `## Benchmark & Size Comparison

### Criterion Benchmarks (base vs PR)

${benchmarkTable}

### Cross-Library Comparison

${crossComparisonTable}

### Binary Size

${sizeTable}

### Memory (Peak RSS)

${rssTable}

### Tests

${testTable}

---
<sub>Threshold: ${threshold}% \u00B7 Regression: ${regressionStatus}</sub>`;
};
