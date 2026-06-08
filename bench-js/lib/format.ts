// bench-js/lib/format.ts
import prettyBytes from 'pretty-bytes';

export const formatNanoseconds = (ns: number): string => {
  if (ns >= 1e9) return `${(ns / 1e9).toFixed(2)} s`;
  if (ns >= 1e6) return `${(ns / 1e6).toFixed(2)} ms`;
  if (ns >= 1e3) return `${(ns / 1e3).toFixed(1)} µs`;
  return `${ns.toFixed(1)} ns`;
};

export const formatBytes = (bytes: number): string => prettyBytes(bytes);
