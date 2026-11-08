/** Guard: every Rust and production JS dependency has a licence on the allow-list (docs/DESIGN.md section 8). */
import { existsSync } from 'node:fs';
import { resolve } from 'node:path';
import { ROOT, run } from '../lib/util.mjs';

const ALLOWED = new Set([
  'MIT', 'Apache-2.0', 'Apache-2.0 WITH LLVM-exception', 'BSD-2-Clause', 'BSD-3-Clause', 'ISC', 'Zlib',
  'MPL-2.0', 'Unicode-3.0', 'Unicode-DFS-2016', 'Unlicense', '0BSD', 'CC0-1.0', 'BSL-1.0',
  'LGPL-2.1-or-later', 'LGPL-3.0-or-later', 'GPL-2.0-or-later', 'GPL-3.0-or-later', 'GPL-3.0-only',
  'OFL-1.1', 'CC-BY-4.0', 'CC-BY-SA-4.0', 'MIT-0', 'BlueOak-1.0.0', 'Python-2.0', 'CDLA-Permissive-2.0',
]);

// Packages with a missing/unparseable licence that we accepted on purpose. Key "name@version" or "name".
// Always add a reason. Empty for now.
const EXCEPTIONS = {
  // 'some-crate': 'licence file is MIT, metadata field missing (checked 2026-11-08)',
  '7z-wasm@1.2.0':
    'only imported by tauri-plugin-libmpv-api dist-js/cli.* (its setup-lib command, which we never run: ' +
    'scripts/fetch-deps.mjs replaces it); not reachable from src/, so never bundled (checked 2026-11-08)',
};

/** Parse an SPDX-ish expression and say whether the allow-list satisfies it. "/" counts as OR. */
export function satisfies(expr) {
  const toks = expr
    .replace(/\//g, ' OR ')
    .replace(/([()])/g, ' $1 ')
    .split(/\s+/)
    .filter(Boolean)
    .map((t) => (/^(and|or|with)$/i.test(t) ? t.toUpperCase() : t));
  let i = 0;
  const atom = () => {
    if (toks[i] === '(') {
      i++;
      const v = or();
      if (toks[i] !== ')') throw new Error('unbalanced parentheses');
      i++;
      return v;
    }
    let id = toks[i++];
    if (!id) throw new Error('unexpected end');
    id = id.replace(/\*$/, '').replace(/^([A-Za-z-]+-\d(?:\.\d)?)\+$/, '$1-or-later');
    if (toks[i] === 'WITH') { i++; id += ' WITH ' + toks[i++]; }
    return ALLOWED.has(id);
  };
  const and = () => { let v = atom(); while (toks[i] === 'AND') { i++; const r = atom(); v = v && r; } return v; };
  const or = () => { let v = and(); while (toks[i] === 'OR') { i++; const r = and(); v = v || r; } return v; };
  const v = or();
  if (i < toks.length) throw new Error('trailing tokens');
  return v;
}

const bad = [];
const check = (name, version, lic) => {
  if (EXCEPTIONS[`${name}@${version}`] || EXCEPTIONS[name]) return;
  let ok = false;
  try { ok = lic ? satisfies(lic) : false; } catch { ok = false; }
  if (!ok) bad.push(`${name}@${version}  ${lic || '(no licence field)'}`);
};
let rust = 0;
let js = 0;
const notes = [];

// Rust
if (existsSync(resolve(ROOT, 'Cargo.toml'))) {
  const r = run('cargo', ['metadata', '--format-version', '1', '--locked']);
  if (r.status !== 0) {
    console.error('cargo metadata --locked failed:\n' + r.stderr.split('\n').slice(0, 15).join('\n'));
    process.exit(1);
  }
  for (const p of JSON.parse(r.stdout).packages) {
    if (p.source === null) continue; // our own workspace crates
    rust++;
    check(p.name, p.version, p.license);
  }
} else notes.push('no Cargo.toml');

// JS (pnpm prints { "<licence>": [ { name, versions: [...] }, ... ] }, or plain text when empty)
if (existsSync(resolve(ROOT, 'package.json'))) {
  const r = run('pnpm', ['licenses', 'list', '--json', '--prod']);
  let data = null;
  try { data = JSON.parse(r.stdout); } catch { /* plain text => nothing to list */ }
  if (r.status !== 0 && /ERR_PNPM_LICENSES_NO_LOCKFILE/.test(r.out)) notes.push('no pnpm-lock.yaml');
  else if (r.status !== 0 || data === null && r.stdout.trim().startsWith('{')) {
    console.error('pnpm licenses failed:\n' + r.out.split('\n').slice(0, 15).join('\n'));
    process.exit(1);
  }
  for (const [lic, pkgs] of Object.entries(r.status === 0 ? data ?? {} : {})) {
    for (const p of pkgs) for (const v of p.versions?.length ? p.versions : ['?']) { js++; check(p.name, v, lic === 'Unknown' ? '' : lic); }
  }
} else notes.push('no package.json');

if (bad.length) {
  console.error('licence not on allow-list:\n' + bad.join('\n'));
  process.exit(1);
}
console.log(`licenses: ok (${rust} crates, ${js} js packages${notes.length ? '; ' + notes.join(', ') : ''})`);
