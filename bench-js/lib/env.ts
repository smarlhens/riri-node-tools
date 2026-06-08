// bench-js/lib/env.ts
import envinfo from 'envinfo';
import { execFileSync } from 'node:child_process';

interface EnvinfoVersion {
  version?: string;
}
interface EnvinfoPackage {
  installed?: string;
}
export interface EnvinfoJson {
  System?: { OS?: string; CPU?: string; Memory?: string };
  Binaries?: { Node?: EnvinfoVersion; npm?: EnvinfoVersion };
  npmPackages?: Record<string, EnvinfoPackage>;
}

export interface Env {
  os: string;
  arch: string;
  cpu: string;
  memory: string;
  node: string;
  npm: string;
  tinybench: string;
  packages: Record<string, string | undefined>;
  rust?: string;
  cargo?: string;
  timestamp?: string;
}

const tryCmd = (cmd: string, args: string[]): string => {
  try {
    return execFileSync(cmd, args, { encoding: 'utf8' }).trim();
  } catch {
    return 'n/a';
  }
};

export const normalizeEnvinfo = (info: EnvinfoJson): Env => ({
  os: info.System?.OS ?? 'n/a',
  arch: process.arch,
  cpu: info.System?.CPU ?? 'n/a',
  memory: info.System?.Memory ?? 'n/a',
  node: info.Binaries?.Node?.version ?? process.versions.node,
  npm: info.Binaries?.npm?.version ?? 'n/a',
  tinybench: info.npmPackages?.tinybench?.installed ?? 'n/a',
  packages: Object.fromEntries(Object.entries(info.npmPackages ?? {}).map(([k, v]) => [k, v.installed])),
});

export const detectEnv = async (): Promise<Env> => {
  const raw = await envinfo.run(
    {
      System: ['OS', 'CPU', 'Memory'],
      Binaries: ['Node', 'npm'],
      npmPackages: ['@smarlhens/*', 'tinybench'],
    },
    { json: true, showNotFound: true },
  );
  const env = normalizeEnvinfo(JSON.parse(raw));
  env.rust = tryCmd('rustc', ['--version']);
  env.cargo = tryCmd('cargo', ['--version']);
  env.timestamp = new Date().toISOString();
  return env;
};
