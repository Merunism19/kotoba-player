/** Guard: source code may only talk to loopback (AnkiConnect, mpv) and the DeepL module; no other URLs or HTTP clients. */
import { readFileSync } from 'node:fs';
import { extname, resolve } from 'node:path';
import { ROOT, repoFiles } from '../lib/util.mjs';

const SCAN_DIRS = ['src/', 'src-tauri/src/', 'crates/'];
const EXTS = new Set(['ts', 'tsx', 'js', 'jsx', 'mjs', 'cjs', 'rs', 'html', 'css']);

// Hosts any file may mention.
const ALLOWED_HOSTS = [
  '127.0.0.1', 'localhost',
  'schema.tauri.app', // JSON schema URL in tauri config
  'www.w3.org', // XML/SVG namespace strings; never fetched
];
// Hosts allowed only in files whose path contains the given word.
const FILE_SCOPED_HOSTS = { 'api-free.deepl.com': 'deepl', 'api.deepl.com': 'deepl' };
// Files whose path contains one of these words may use an HTTP client (AnkiConnect is loopback, DeepL is opt-in).
const CLIENT_FILE_WORDS = ['deepl', 'anki'];
// HTTP client usage patterns.
const CLIENT_RE = /\b(fetch\s*\(|XMLHttpRequest|new\s+WebSocket|axios|reqwest|ureq|hyper::|isahc|attohttpc|tauri_plugin_http|sendBeacon)/;
const URL_RE = /https?:\/\/[^\s"'`)<>\]\\]+/g;
const COMMENT_RE = /^\s*(\/\/|\/\*|\*)/; // line, doc and block comment lines may hold doc links
// Escape hatch: `// network-guard: ok <reason>` on the same or the previous line.
const OK_RE = /network-guard:\s*ok/;

const problems = [];
let scanned = 0;
for (const f of repoFiles()) {
  if (!SCAN_DIRS.some((d) => f.startsWith(d)) || !EXTS.has(extname(f).slice(1))) continue;
  scanned++;
  const lines = readFileSync(resolve(ROOT, f), 'utf8').split('\n');
  const lower = f.toLowerCase();
  lines.forEach((text, idx) => {
    if (OK_RE.test(text) || (idx > 0 && OK_RE.test(lines[idx - 1]))) return;
    if (COMMENT_RE.test(text)) return;
    for (const m of text.matchAll(URL_RE)) {
      let host;
      try { host = new URL(m[0]).hostname; } catch { host = m[0]; }
      if (ALLOWED_HOSTS.includes(host)) continue;
      const scope = FILE_SCOPED_HOSTS[host];
      if (scope && lower.includes(scope)) continue;
      problems.push(`${f}:${idx + 1} URL to ${host}`);
    }
    if (CLIENT_RE.test(text) && !CLIENT_FILE_WORDS.some((w) => lower.includes(w))) {
      problems.push(`${f}:${idx + 1} HTTP client usage outside deepl/anki modules`);
    }
  });
}

if (problems.length) {
  console.error(problems.join('\n') + '\n(allowed: see scripts/guards/network.mjs; per-line override: // network-guard: ok <reason>)');
  process.exit(1);
}
console.log(`network: ok (${scanned} files)`);
