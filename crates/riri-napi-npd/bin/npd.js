#!/usr/bin/env node
// Thin shim: forwards argv to the Rust CLI compiled as a NAPI addon.
// Output (stdout/stderr) is written by Rust directly to the host process.

const { runCli } = require('..');

const code = runCli(['npd', ...process.argv.slice(2)]);
process.exit(code);
