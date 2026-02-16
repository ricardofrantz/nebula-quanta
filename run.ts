#!/usr/bin/env bun

import { argv } from 'node:process';
import { spawnSync } from 'node:child_process';

const args = argv.slice(2);
const result = spawnSync('cargo', ['run', '--', ...args], { stdio: 'inherit' });
if (result.error) {
  console.error(`failed to run cargo: ${result.error.message}`);
  process.exit(1);
}
process.exit(result.status ?? 0);
