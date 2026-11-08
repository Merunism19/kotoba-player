/** Guard: every text file is valid UTF-8 with LF endings, no BOM and no stray control characters. */
import { readFileSync } from 'node:fs';
import { extname, resolve } from 'node:path';
import { ROOT, repoFiles } from '../lib/util.mjs';

const SKIP = new Set('png jpg webp ico icns mkv mp4 dll exe sqlite woff woff2 ttf otf 7z zip'.split(' '));
const dec = new TextDecoder('utf-8', { fatal: true });
const problems = [];
let checked = 0;

for (const f of repoFiles()) {
  if (SKIP.has(extname(f).slice(1).toLowerCase())) continue;
  const buf = readFileSync(resolve(ROOT, f));
  checked++;
  if (buf.length >= 3 && buf[0] === 0xef && buf[1] === 0xbb && buf[2] === 0xbf) problems.push(`${f}:1 UTF-8 BOM`);
  let line = 1;
  let start = 0;
  const reported = new Set();
  const note = (kind) => {
    const k = kind + line;
    if (!reported.has(k)) { reported.add(k); problems.push(`${f}:${line} ${kind}`); }
  };
  for (let i = 0; i <= buf.length; i++) {
    const b = i < buf.length ? buf[i] : 10; // virtual final newline
    if (b === 13) note('CR byte (use LF)');
    else if ((b < 32 && b !== 9 && b !== 10) || b === 127) note(`control char 0x${b.toString(16).padStart(2, '0')}`);
    if (b === 10) {
      try { dec.decode(buf.subarray(start, i)); } catch { note('invalid UTF-8'); }
      line++;
      start = i + 1;
    }
  }
}

if (problems.length) {
  console.error(problems.join('\n'));
  process.exit(1);
}
console.log(`encoding: ok (${checked} files)`);
