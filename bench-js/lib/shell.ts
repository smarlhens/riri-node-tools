// bench-js/lib/shell.ts
import { execFileSync } from 'node:child_process';

export interface HyperfineResult {
  command: string;
  mean: number;
  stddev: number;
}
export interface HyperfineJson {
  results: HyperfineResult[];
}
export interface CliRow {
  fixture: string;
  variant: string;
  mean_s: number;
  stddev_s: number;
}
export interface SizeRow {
  binary: string;
  bytes: number;
}
export interface MemoryUsage {
  peak_kb: number;
  total_kb: number;
}

export const parseHyperfine = (json: HyperfineJson, fixture: string): CliRow[] =>
  json.results.map(r => ({ fixture, variant: r.command, mean_s: r.mean, stddev_s: r.stddev }));

export const parseSize = (stdout: string): SizeRow[] =>
  stdout
    .split('\n')
    .map(l => l.trim())
    .filter(Boolean)
    .map(l => {
      const [binary, bytes] = l.split('\t');
      return { binary, bytes: Number(bytes) };
    });

export const parseMemory = (stdout: string): MemoryUsage => {
  const get = (key: string): number => Number((stdout.match(new RegExp(`${key}=(\\d+)`)) ?? [])[1] ?? 0);
  return { peak_kb: get('peak_kb'), total_kb: get('total_kb') };
};

export const runScript = (scriptPath: string, args: string[] = []): string =>
  execFileSync('bash', [scriptPath, ...args], { encoding: 'utf8' });
