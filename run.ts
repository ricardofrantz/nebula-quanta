#!/usr/bin/env bun

import { argv } from 'node:process';
import { spawnSync } from 'node:child_process';
import { existsSync, rmSync } from 'node:fs';
import { dirname, resolve } from 'node:path';

type Preset = 'fast' | 'balanced' | 'accurate';

type PresetMap = Record<Preset, string[]>;

type MetricLine = Record<string, string>;

const PRESETS: PresetMap = {
  fast: ['--integrator', 'leapfrog', '--theta', '0.9', '--dt', '0.0016', '--threads', '1'],
  balanced: ['--integrator', 'leapfrog', '--theta', '0.6', '--dt', '0.0010', '--threads', '2'],
  accurate: ['--integrator', 'rk2', '--theta', '0.35', '--dt', '0.0007', '--threads', '4'],
};

const parseMetricLine = (line: string): MetricLine => {
  const fields: MetricLine = {};
  for (const token of line.split(' ')) {
    const eq = token.indexOf('=');
    if (eq === -1) {
      continue;
    }
    const key = token.slice(0, eq);
    const value = token.slice(eq + 1);
    if (key.length > 0 && value.length > 0) {
      fields[key] = value;
    }
  }
  return fields;
};

const pct = (value: number, total: number): string => {
  if (!Number.isFinite(value) || !Number.isFinite(total) || total <= 0) {
    return 'na';
  }
  return `${((value / total) * 100).toFixed(1)}%`;
};

const formatPerfSummary = (summaryLine: string): string => {
  const fields = parseMetricLine(summaryLine);

  const get = (key: string): string => fields[key] ?? 'na';
  const totalMs = Number(get('total_ms'));
  const buildMs = Number(get('build_ms'));
  const forceMs = Number(get('force_ms'));
  const integrateMs = Number(get('integrate_ms'));

  const perfLine = `[perf] ${get('mode')} n=${get('n')} steps=${get('steps')} threads=${get('threads')} theta=${get('theta')} dt=${get('dt')} total_ms=${get('total_ms')} avg_step_ms=${get('avg_step_ms')} steps_per_sec=${get('steps_per_sec')} ns_per_particle_force=${get('ns_per_particle_force')}`;

  const shareLine = `[perf] build=${get('build_ms')}ms (${pct(buildMs, totalMs)}) force=${get('force_ms')}ms (${pct(forceMs, totalMs)}) integrate=${get('integrate_ms')}ms (${pct(integrateMs, totalMs)}) workspace_bytes=${get('workspace_bytes')} peak_nodes=${get('peak_nodes')} node_utilization=${get('node_utilization')}`;

  return `${perfLine}\n${shareLine}`;
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

type RenderMeta = {
  fps: string;
  input: string;
  resolution: string;
  output: string;
  frames: string;
};

const extractRenderMetadata = (stdout: string): RenderMeta | null => {
  const recordLine = stdout
    .split('\n')
    .find((line) => line.startsWith('record_frames='));
  if (!recordLine) {
    return null;
  }

  const match = recordLine.match(/render_cmd="(.+?)"\s*$/);
  if (!match) {
    return null;
  }

  const tokens = match[1].split(/\s+/);
  if (tokens.length < 8) {
    return null;
  }

  const renderOutput = tokens.at(-1);
  if (!renderOutput || renderOutput.length === 0) {
    return null;
  }

  const framesMatch = recordLine.match(/record_frames=([0-9]+)/);
  const frames = framesMatch?.[1] ?? 'na';
  let fps = '60';
  let input = '';
  let resolution = '';

  for (let i = 0; i < tokens.length; i += 1) {
    const token = tokens[i];
    if (token === '-framerate' && tokens[i + 1]) {
      fps = tokens[i + 1];
    } else if (token === '-i' && tokens[i + 1]) {
      input = tokens[i + 1];
    } else if (token === '-s' && tokens[i + 1]) {
      resolution = tokens[i + 1];
    }
  }

  if (!fps || !input || !resolution) {
    return null;
  }

  return {
    fps,
    input,
    resolution,
    output: renderOutput,
    frames,
  };
};

const VALUE_OPTIONS = new Set([
  '--mode',
  '--n',
  '--steps',
  '--dt',
  '--theta',
  '--theta-policy',
  '--theta-density-scale',
  '--softening-policy',
  '--softening-density-scale',
  '--epsilon',
  '--g',
  '--integrator',
  '--init',
  '--init-radius',
  '--init-spread',
  '--init-v-amp',
  '--init-lambda',
  '--init-center-x',
  '--init-center-y',
  '--mass-profile',
  '--mass-mean',
  '--mass-stddev',
  '--mass-min',
  '--mass-max',
  '--mass-alpha',
  '--seed',
  '--energy-drift',
  '--energy-sample-ratio',
  '--dim',
  '--threads',
  '--preset',
  '--max-memory-mib',
  '--frames-dir',
  '--width',
  '--height',
  '--fps',
  '--every-steps',
]);

const BOOL_OPTIONS = new Set(['--validate', '--record']);

const dedupeLastWins = (args: string[]): string[] => {
  const reversed: string[] = [];
  const seen = new Set<string>();

  for (let i = args.length - 1; i >= 0; i -= 1) {
    const token = args[i];
    if (!token.startsWith('--')) {
      reversed.push(token);
      continue;
    }

    const equals = token.indexOf('=');
    const key = equals === -1 ? token : token.slice(0, equals);
    const expectsValue = equals === -1 && VALUE_OPTIONS.has(key);
    const isBoolean = equals === -1 && BOOL_OPTIONS.has(key);

    if (!VALUE_OPTIONS.has(key) && !isBoolean) {
      reversed.push(token);
      continue;
    }

    if (seen.has(key)) {
      if (expectsValue && reversed.length > 0) {
        reversed.pop();
      }
      continue;
    }

    seen.add(key);
    reversed.push(token);
  }

  return reversed.reverse();
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

const finalSimArgs = dedupeLastWins([...presetArgs, ...simArgs]);
const hasExplicitRecordFlag = finalSimArgs.some(
  (arg) => arg === '--record' || arg.startsWith('--record='),
);

if (!hasExplicitRecordFlag) {
  finalSimArgs.push('--record', '--frames-dir', 'frames', '--width', '1920', '--height', '1080', '--fps', '60', '--every-steps', '1');
}

const doSave = true;

const cargoArgs = ['run'];
if (useRelease) {
  cargoArgs.push('--release');
}
cargoArgs.push('--', ...finalSimArgs);

const cargoCommand = process.env.CARGO_BIN || 'cargo';

const result = spawnSync(cargoCommand, cargoArgs, {
  encoding: 'utf8',
  maxBuffer: 32 * 1024 * 1024,
  stdio: ['ignore', 'pipe', 'pipe'],
});
if (result.error) {
  console.error(`failed to run cargo: ${result.error.message}`);
  process.exit(1);
}

if (typeof result.stdout === 'string' && result.stdout.length > 0) {
  process.stdout.write(result.stdout);
  const summaryLine = result.stdout
    .split('\n')
    .find((line) => line.startsWith('mode='));
  if (summaryLine) {
    try {
      console.log(formatPerfSummary(summaryLine));
    } catch (error) {
      console.error(`unable to render perf summary: ${(error as Error).message}`);
    }
  }
}

if (typeof result.stderr === 'string' && result.stderr.length > 0) {
  process.stderr.write(result.stderr);
}

if (doSave) {
  const renderMeta = result.stdout ? extractRenderMetadata(result.stdout as string) : null;
  if (!renderMeta) {
    console.error('recording output metadata missing; cannot auto-render mp4');
    process.exit(1);
  }

  const ffmpegArgs = [
    '-y',
    '-hide_banner',
    '-loglevel',
    'error',
    '-framerate',
    renderMeta.fps,
    '-i',
    renderMeta.input,
    '-s',
    renderMeta.resolution,
    '-c:v',
    'libx264',
    '-pix_fmt',
    'yuv420p',
    renderMeta.output,
  ];

  const renderStartMs = Date.now();
  const ffmpegResult = spawnSync('ffmpeg', ffmpegArgs, {
    encoding: 'utf8',
    maxBuffer: 32 * 1024 * 1024,
    stdio: 'inherit',
  });

  const cleanupFrameDirs = (inputPath: string) => {
    const primaryDir = dirname(inputPath);
    const candidates = new Set<string>([primaryDir, 'frames', 'frames_preview']);
    const normalizedPrimary = primaryDir ? resolve(primaryDir) : '';
    for (const candidate of candidates) {
      if (!candidate || candidate === '.') {
        continue;
      }
      const normalizedCandidate = resolve(candidate);
      if (normalizedCandidate === normalizedPrimary && !existsSync(candidate)) {
        continue;
      }
      if (!existsSync(candidate)) {
        continue;
      }
      try {
        rmSync(candidate, { recursive: true, force: true });
        console.log(`frames cleaned : ${candidate}`);
      } catch (cleanupError) {
        console.error(`unable to clean frames_dir ${candidate}: ${(cleanupError as Error).message}`);
      }
    }
  };

  if (ffmpegResult.error) {
    console.error(`failed to run ffmpeg: ${ffmpegResult.error.message}`);
    cleanupFrameDirs(renderMeta.input);
    process.exit(1);
  }

  if ((ffmpegResult.status ?? 1) !== 0) {
    console.error(`ffmpeg exited with code ${(ffmpegResult.status ?? 1)} while saving ${renderMeta.output}`);
    cleanupFrameDirs(renderMeta.input);
    process.exit(ffmpegResult.status ?? 1);
  }

  const renderMs = Date.now() - renderStartMs;
  console.log(
    `[perf] render output=${renderMeta.output} frames=${renderMeta.frames} fps=${renderMeta.fps} encode_ms=${renderMs} resolution=${renderMeta.resolution}`,
  );
  console.log(`Saving : ${renderMeta.output}`);

  cleanupFrameDirs(renderMeta.input);
}

process.exit(result.status ?? 0);
