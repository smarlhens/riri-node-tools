import type { BenchmarkResult } from "./criterion.js";

function formatNs(ns: number): string {
  if (ns >= 1_000_000_000) return `${(ns / 1_000_000_000).toFixed(2)} s`;
  if (ns >= 1_000_000) return `${(ns / 1_000_000).toFixed(2)} ms`;
  if (ns >= 1_000) return `${(ns / 1_000).toFixed(1)} us`;
  return `${ns.toFixed(1)} ns`;
}

function formatBytes(bytes: number): string {
  const abs = Math.abs(bytes);
  if (abs >= 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MiB`;
  if (abs >= 1024) return `${(bytes / 1024).toFixed(1)} KiB`;
  return `${bytes} B`;
}

function formatPct(pct: number): string {
  const sign = pct > 0 ? "+" : "";
  return `${sign}${pct.toFixed(2)}%`;
}

function regressionIcon(pct: number, threshold: number): string {
  if (pct > threshold) return ":x:";
  if (pct > 0) return ":warning:";
  if (pct < -1) return ":white_check_mark:";
  return "";
}

export function formatBenchmarkTable(results: BenchmarkResult[], threshold: number): string {
  if (results.length === 0) return "_No criterion benchmarks found._";

  const rows = results
    .map((r) => {
      const icon = regressionIcon(r.diffPct, threshold);
      return `| ${r.name} | ${formatNs(r.baseNs)} | ${formatNs(r.prNs)} | ${formatPct(r.diffPct)} | ${icon} |`;
    })
    .join("\n");

  return `| Benchmark | Base | PR | \u0394 | |
|-----------|------|----|---|---|
${rows}`;
}

export function formatBinarySizeTable(prSizes: Record<string, number>, baseSizes: Record<string, number>): string {
  const binaries = [...new Set([...Object.keys(prSizes), ...Object.keys(baseSizes)])].sort();

  if (binaries.length === 0) return "_No binary size data._";

  const rows = binaries
    .map((name) => {
      const pr = prSizes[name];
      const base = baseSizes[name];

      if (pr != null && base != null) {
        const diff = pr - base;
        const pct = base === 0 ? 0 : (diff / base) * 100;
        return `| \`${name}\` | ${formatBytes(base)} | ${formatBytes(pr)} | ${formatBytes(diff)} | ${formatPct(pct)} |`;
      }
      if (pr != null) {
        return `| \`${name}\` | _new_ | ${formatBytes(pr)} | - | - |`;
      }
      return `| \`${name}\` | ${formatBytes(base!)} | _removed_ | - | - |`;
    })
    .join("\n");

  return `| Binary | Base | PR | Diff | % |
|--------|------|----|------|---|
${rows}`;
}

export function formatRssTable(prRssKb: number, baseRssKb: number): string {
  if (prRssKb === 0 && baseRssKb === 0) return "_No memory data._";

  const diffKb = prRssKb - baseRssKb;
  const pct = baseRssKb === 0 ? 0 : (diffKb / baseRssKb) * 100;

  return `| Metric | Base | PR | Diff | % |
|--------|------|----|------|---|
| Peak RSS | ${formatBytes(baseRssKb * 1024)} | ${formatBytes(prRssKb * 1024)} | ${formatBytes(diffKb * 1024)} | ${formatPct(pct)} |`;
}

export function formatTestCountTable(prCount: number, baseCount: number): string {
  const diff = prCount - baseCount;
  const diffStr = diff > 0 ? `+${diff}` : diff === 0 ? "0" : `${diff}`;

  return `| | Base | PR | Diff |
|--|------|----|------|
| Test count | ${baseCount} | ${prCount} | ${diffStr} |`;
}

export function buildFullComment(
  benchmarkTable: string,
  sizeTable: string,
  rssTable: string,
  testTable: string,
  threshold: number,
  maxRegression: number,
): string {
  const regressionStatus =
    maxRegression > threshold ? `**regression detected: ${maxRegression.toFixed(1)}%**` : "none detected";

  return `## Benchmark & Size Comparison

### Criterion Benchmarks

${benchmarkTable}

### Binary Size

${sizeTable}

### Memory (Peak RSS)

${rssTable}

### Tests

${testTable}

---
<sub>Threshold: ${threshold}% \u00b7 Regression: ${regressionStatus}</sub>`;
}
