#!/usr/bin/env bun

import { argv } from 'node:process';
import { spawnSync } from 'node:child_process';

type Preset = 'fast' | 'balanced' | 'accurate';

type PresetMap = Record<Preset, string[]>;

const PRESETS: PresetMap = {
  fast: ['--integrator', 'leapfrog', '--theta', '0.9', '--dt', '0.0016', '--threads', '1'],
  balanced: ['--integrator', 'leapfrog', '--theta', '0.6', '--dt', '0.0010', '--threads', '2'],
  accurate: ['--integrator', 'rk2', '--theta', '0.35', '--dt', '0.0007', '--threads', '4'],
};

const args = argv.slice(2);
let useRelease = true;
const presetArgs: string[] = [];
const simArgs: string[] = [];

const parsePreset = (value: string): Preset | '' => {
  if (value === 'fast' || value === 'balanced' || value === 'accurate') {
    return value;
  }
  return '';
};

for (let i = 0; i < args.length; i += 1) {
  const arg = args[i];
  if (arg === '--release') {
    useRelease = true;
    continue;
  }
  if (arg === '--debug') {
    useRelease = false;
    continue;
  }

  if (arg === '--preset') {
    const value = args[i + 1] ?? '';
    if (value === '') {
      console.error('--preset requires one value: fast|balanced|accurate');
      process.exit(1);
    }
    const preset = parsePreset(value);
    if (!preset) {
      console.error(`unknown preset: ${value}. expected fast|balanced|accurate`);
      process.exit(1);
    }
    presetArgs.length = 0;
    presetArgs.push(...PRESETS[preset]);
    i += 1;
    continue;
  }

  if (arg.startsWith('--preset=')) {
    const value = arg.slice('--preset='.length);
    const preset = parsePreset(value);
    if (!preset) {
      console.error(`unknown preset: ${value}. expected fast|balanced|accurate`);
      process.exit(1);
    }
    presetArgs.length = 0;
    presetArgs.push(...PRESETS[preset]);
    continue;
  }

  simArgs.push(arg);
}

const cargoArgs = ['run'];
if (useRelease) {
  cargoArgs.push('--release');
}
cargoArgs.push('--', ...presetArgs, ...simArgs);

const result = spawnSync('cargo', cargoArgs, { stdio: 'inherit' });
if (result.error) {
  console.error(`failed to run cargo: ${result.error.message}`);
  process.exit(1);
}
process.exit(result.status ?? 0);
