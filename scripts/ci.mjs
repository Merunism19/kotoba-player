/** Local CI: runs every gate in order, stops at the first failure, ends with `CI OK` or `CI FAILED: <step>`. */
import { existsSync, readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { ROOT, run } from './lib/util.mjs';

const args = new Set(process.argv.slice(2));
const quick = args.has('--quick');
const docsOnly = args.has('--docs-only');
const has = (p) => existsSync(resolve(ROOT, p));
const hasBuildScript = () => {
  try { return Boolean(JSON.parse(readFileSync(resolve(ROOT, 'package.json'), 'utf8')).scripts?.build); } catch { return false; }
};

const node = (script, ...a) => ({ cmd: 'node', args: [script, ...a] });
// skip: reason string => step is not applicable yet; only: step kept in the given modes
const steps = [
  { name: 'encoding', ...node('scripts/guards/encoding.mjs'), docs: true },
  { name: 'network', ...node('scripts/guards/network.mjs') },
  { name: 'stores', ...node('scripts/guards/stores.mjs') },
  { name: 'mpv', ...node('scripts/guards/mpv.mjs') },
  { name: 'licenses', ...node('scripts/guards/licenses.mjs'), slow: true },
  { name: 'codemap', ...node('scripts/codemap.mjs', '--check'), docs: true },
  { name: 'tsc', cmd: 'pnpm', args: ['-s', 'exec', 'tsc', '--noEmit'], skip: () => !has('tsconfig.json') && 'no tsconfig.json' },
  { name: 'vite build', cmd: 'pnpm', args: ['-s', 'build'], slow: true, skip: () => !hasBuildScript() && 'no build script' },
  { name: 'cargo fmt', cmd: 'cargo', args: ['fmt', '--all', '--check'], skip: () => !has('Cargo.toml') && 'no Cargo.toml' },
  {
    name: 'deps',
    skip: () => !has('src-tauri/Cargo.toml') && 'no src-tauri',
    check: () => (has('src-tauri/lib') ? null : 'run node scripts/fetch-deps.mjs first'),
  },
  { name: 'cargo clippy', cmd: 'cargo', args: ['clippy', '-q', '--workspace', '--all-targets', '--', '-D', 'warnings'], slow: true, skip: () => !has('Cargo.toml') && 'no Cargo.toml' },
  { name: 'cargo test', cmd: 'cargo', args: ['test', '-q', '--workspace'], skip: () => !has('Cargo.toml') && 'no Cargo.toml' },
  // Headless screenshots of every screen (mock mode) + UI behaviour tests; images in artifacts/shots/.
  { name: 'shots', cmd: 'pnpm', args: ['-s', 'shots'], slow: true, skip: () => !has('playwright.config.ts') && 'no playwright config' },
];

function tail(text, n = 40) {
  const lines = text.replace(/\x1b\[[0-9;]*m/g, '').split('\n').filter((l) => l.trim() !== '');
  return (lines.length > n ? ['… (' + (lines.length - n) + ' earlier lines omitted)', ...lines.slice(-n)] : lines).join('\n');
}

let failed = null;
for (const s of steps) {
  if (docsOnly && !s.docs) continue;
  if (quick && s.slow) continue;
  const skipReason = s.skip?.();
  if (skipReason) { console.log(`skip  ${s.name} (${skipReason})`); continue; }
  const t0 = Date.now();
  let ok;
  let output = '';
  if (s.check) {
    const msg = s.check();
    ok = msg === null;
    output = msg ?? '';
  } else {
    const r = run(s.cmd, s.args, { env: { ...process.env, CARGO_TERM_COLOR: 'never', NO_COLOR: '1' } });
    ok = r.status === 0;
    output = r.out || (r.error ? String(r.error) : '');
  }
  const secs = ((Date.now() - t0) / 1000).toFixed(1);
  if (ok) console.log(`ok    ${s.name} (${secs}s)`);
  else {
    console.log(`FAIL  ${s.name} (${secs}s)`);
    console.log(tail(output));
    failed = s.name;
    break;
  }
}
console.log(failed ? `CI FAILED: ${failed}` : 'CI OK');
process.exit(failed ? 1 : 0);
