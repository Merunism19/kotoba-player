/** Guard: persistent stores go through src-tauri/src/stores.rs only (no other Rust code names a .json file or uses localStorage for data). */
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { ROOT, repoFiles } from '../lib/util.mjs';

// The registry itself, and tests (fixtures are .json files).
const ALLOWED = ['src-tauri/src/stores.rs'];
const JSON_FILE_RE = /"[^"\n]*\.json"/; // a string literal naming a .json file
const TEST_START_RE = /#\[cfg\(test\)\]/;
// UI: localStorage/IndexedDB are for per-viewer conveniences only; data belongs to the Rust stores.
const UI_STORE_RE = /\b(localStorage|indexedDB)\b/;
const OK_RE = /stores-guard:\s*ok/;

const problems = [];
for (const f of repoFiles()) {
  const isRust = f.startsWith('src-tauri/src/') && f.endsWith('.rs');
  const isUi = f.startsWith('src/') && /\.(ts|tsx)$/.test(f);
  if ((!isRust && !isUi) || ALLOWED.includes(f)) continue;
  const lines = readFileSync(resolve(ROOT, f), 'utf8').split('\n');
  for (let i = 0; i < lines.length; i++) {
    if (isRust && TEST_START_RE.test(lines[i])) break; // test modules may name fixtures
    const ok = OK_RE.test(lines[i]) || (i > 0 && OK_RE.test(lines[i - 1]));
    if (ok) continue;
    if (isRust && JSON_FILE_RE.test(lines[i])) problems.push(`${f}:${i + 1}: names a .json file outside stores.rs (add a Store variant instead)`);
    if (isUi && UI_STORE_RE.test(lines[i])) problems.push(`${f}:${i + 1}: browser storage for app data (use a Rust store; mark UI-only conveniences with // stores-guard: ok <reason>)`);
  }
}
if (problems.length) {
  console.error(problems.join('\n'));
  process.exit(1);
}
