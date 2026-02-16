#!/usr/bin/env bun

import { argv } from 'node:process';
import { spawnSync } from 'node:child_process';

const args = argv.slice(2);
let useRelease = true;
const simArgs: string[] = [];

for (const arg of args) {
  if (arg === '--release') {
    useRelease = true;
    continue;
  }
  if (arg === '--debug') {
    useRelease = false;
    continue;
  }
  simArgs.push(arg);
}

const cargoArgs = ['run'];
if (useRelease) {
  cargoArgs.push('--release');
}
cargoArgs.push('--', ...simArgs);

const result = spawnSync('cargo', cargoArgs, { stdio: 'inherit' });
if (result.error) {
  console.error(`failed to run cargo: ${result.error.message}`);
  process.exit(1);
}
process.exit(result.status ?? 0);
