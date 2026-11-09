/** Guard: UI code never asks the libmpv plugin for the 'node' format (it segfaults the app; read node properties as 'string' JSON). */
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { ROOT, repoFiles } from '../lib/util.mjs';

const NODE_FORMAT_RE = /['"`]node['"`]/;
const problems = [];
for (const f of repoFiles()) {
  if (!f.startsWith('src/') || !/\.(ts|tsx)$/.test(f)) continue;
  const text = readFileSync(resolve(ROOT, f), 'utf8');
  if (!text.includes('tauri-plugin-libmpv-api')) continue;
  text.split('\n').forEach((line, i) => {
    if (NODE_FORMAT_RE.test(line) && !/^\s*(\/\/|\*)/.test(line)) {
      problems.push(`${f}:${i + 1}: mpv 'node' format crashes the app (HANDOFF gotcha); use 'string' and JSON.parse`);
    }
  });
}
if (problems.length) {
  console.error(problems.join('\n'));
  process.exit(1);
}
