/** Guard: mpv is driven only through the app's guarded commands (no plugin init/command/set_property, no 'node' format), and the window never gets the plugin's write permissions. */
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { ROOT, repoFiles } from '../lib/util.mjs';

const NODE_FORMAT_RE = /['"`]node['"`]/;
// Plugin functions that bypass src-tauri/src/mpv_guard.rs (security review 2026-11-08).
const FORBIDDEN_IMPORTS = ['init', 'command', 'setProperty', 'destroy', 'setVideoMarginRatio'];
const RAW_INVOKE_RE = /plugin:libmpv\|(init|command|set_property|destroy)/;
const problems = [];

for (const f of repoFiles()) {
  if (!f.startsWith('src/') || !/\.(ts|tsx)$/.test(f)) continue;
  const text = readFileSync(resolve(ROOT, f), 'utf8');
  const imp = /import\s*\{([^}]*)\}\s*from\s*['"]tauri-plugin-libmpv-api['"]/.exec(text);
  if (imp) {
    for (const name of imp[1].split(',').map((s) => s.trim().split(/\s+as\s+/)[0]).filter(Boolean)) {
      if (FORBIDDEN_IMPORTS.includes(name)) {
        problems.push(`${f}: imports '${name}' from tauri-plugin-libmpv-api; use the guarded mpv_* commands (src/lib/player.ts)`);
      }
    }
  }
  text.split('\n').forEach((line, i) => {
    if (/^\s*(\/\/|\*)/.test(line)) return;
    if (RAW_INVOKE_RE.test(line)) problems.push(`${f}:${i + 1}: raw libmpv plugin write call; use mpv_command/mpv_set`);
    if (text.includes('tauri-plugin-libmpv-api') && NODE_FORMAT_RE.test(line)) {
      problems.push(`${f}:${i + 1}: mpv 'node' format crashes the app (HANDOFF gotcha); use 'string' and JSON.parse`);
    }
  });
}

// The window may read mpv properties and receive events, nothing more from the plugin.
for (const f of repoFiles()) {
  if (!f.startsWith('src-tauri/capabilities/') || !f.endsWith('.json')) continue;
  const perms = JSON.parse(readFileSync(resolve(ROOT, f), 'utf8')).permissions ?? [];
  for (const p of perms) {
    const id = typeof p === 'string' ? p : p.identifier;
    if (/^libmpv:/.test(id) && id !== 'libmpv:allow-get-property') {
      problems.push(`${f}: grants ${id}; only libmpv:allow-get-property is allowed (mpv_guard.rs does the rest)`);
    }
    if (/^opener:/.test(id)) problems.push(`${f}: grants ${id}; the app opens no URLs`);
  }
}

if (problems.length) {
  console.error(problems.join('\n'));
  process.exit(1);
}
