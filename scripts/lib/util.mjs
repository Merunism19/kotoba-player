/** Shared helpers for repo scripts: repo root, process running, file listing. */
import { spawnSync } from 'node:child_process';
import { existsSync } from 'node:fs';
import { dirname, resolve, delimiter } from 'node:path';
import { fileURLToPath } from 'node:url';
import { homedir } from 'node:os';

process.noDeprecation = true; // pnpm .cmd shim needs shell:true on Windows with fixed args
export const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), '..', '..');

// Make `cargo` findable even when the shell PATH lacks ~/.cargo/bin.
const cargoBin = resolve(homedir(), '.cargo', 'bin');
if (existsSync(cargoBin) && !(process.env.PATH ?? process.env.Path ?? '').includes(cargoBin)) {
  const key = process.platform === 'win32' ? 'Path' : 'PATH';
  process.env[key] = cargoBin + delimiter + (process.env[key] ?? '');
}

/** Run a command with an args array (no shell interpolation). pnpm is a .cmd shim on Windows, so it alone uses a shell; its args are fixed literals. */
export function run(cmd, args = [], opts = {}) {
  const shell = process.platform === 'win32' && cmd === 'pnpm';
  const r = spawnSync(cmd, args, { cwd: ROOT, encoding: 'utf8', shell, maxBuffer: 256 * 1024 * 1024, ...opts });
  const out = (r.stdout ?? '') + (r.stderr ?? '');
  return { status: r.status ?? (r.error ? 127 : 1), stdout: r.stdout ?? '', stderr: r.stderr ?? '', out, error: r.error };
}

/** Tracked plus untracked-but-not-ignored files (so planted/new files are checked before commit). */
export function repoFiles() {
  const r = run('git', ['ls-files', '-z', '--cached', '--others', '--exclude-standard']);
  if (r.status !== 0) throw new Error('git ls-files failed: ' + r.out);
  const seen = new Set();
  for (const f of r.stdout.split('\0')) if (f && existsSync(resolve(ROOT, f))) seen.add(f);
  return [...seen].sort((a, b) => (a < b ? -1 : a > b ? 1 : 0));
}

export function sleepSync(ms) {
  Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, ms);
}
