// Downloads the pinned third-party binaries (libmpv, libmpv-wrapper, ffmpeg/ffprobe), checks their SHA-256 and installs them into src-tauri/lib/.
// Also fetches the dictionary sources (JMdict, Kanjium) into vendor-cache/dict-src/ and compiles them into src-tauri/lib/dict/jmdict.sqlite
// with crates/jpdict's build-dict (rebuilt when a source hash or the builder changes).
//
// Usage: node scripts/fetch-deps.mjs [--force]
// Archives are cached in vendor-cache/ (gitignored). Extraction uses Windows' bundled bsdtar
// (C:\Windows\System32\tar.exe reads .7z and .zip). To upgrade a dependency: change its entry
// below (url + sha256 from the GitHub release's digest), run with --force, test, commit.
import { createHash } from 'node:crypto';
import { spawnSync } from 'node:child_process';
import { copyFileSync, existsSync, mkdirSync, readFileSync, rmSync, statSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { run } from './lib/util.mjs'; // also puts ~/.cargo/bin on PATH

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const CACHE = join(ROOT, 'vendor-cache');
const LIB = join(ROOT, 'src-tauri', 'lib');
const STAMP = join(CACHE, 'deps-stamp.json'); // not in lib/: everything there is bundled
const force = process.argv.includes('--force');

/**
 * Pinned dependencies. `files` maps the path inside the archive to the installed file name.
 * `dir` (relative to the repo root) installs somewhere other than src-tauri/lib/; `raw: true` means the URL is the file itself (no archive).
 */
const DEPS = [
  {
    name: 'libmpv (zhongfly mpv-winbuild, LGPL, x86_64 non-v3)',
    url: 'https://github.com/zhongfly/mpv-winbuild/releases/download/2026-10-30-8e5a96fc45/mpv-dev-lgpl-x86_64-20261030-git-8e5a96fc45.7z',
    sha256: '35d45ddb36b978761a38abac6c7635bdb182aa49e6f36e9785df9525c24ea3b1',
    files: { 'libmpv-2.dll': 'libmpv-2.dll' },
  },
  {
    name: 'libmpv-wrapper v0.1.1 (nini22P)',
    url: 'https://github.com/nini22P/libmpv-wrapper/releases/download/v0.1.1/libmpv-wrapper-windows-x86_64.zip',
    sha256: 'd2ff8b2edcd34d2968e544adaa915e5e5c48eb1a0995945005269c2af119a492',
    files: { 'bin/libmpv-wrapper.dll': 'libmpv-wrapper.dll' },
  },
  {
    // Shared build: small exes + DLLs (~165 MB) instead of two static exes (~263 MB). ffplay is not installed.
    name: 'FFmpeg n8.1.3 (BtbN, LGPL, win64 shared)',
    url: 'https://github.com/BtbN/FFmpeg-Builds/releases/download/autobuild-2026-10-30-13-01/ffmpeg-n8.1.3-27-g8dca1586f4-win64-lgpl-shared-8.1.zip',
    sha256: '21b96a5b382b031e89c1c91d7af8efc3db989c4c11bff27422a4c4ea24cf45ed',
    files: Object.fromEntries(
      ['ffmpeg.exe', 'ffprobe.exe', 'avcodec-62.dll', 'avdevice-62.dll', 'avfilter-11.dll', 'avformat-62.dll',
        'avutil-60.dll', 'swresample-6.dll', 'swscale-9.dll']
        .map((f) => [`ffmpeg-n8.1.3-27-g8dca1586f4-win64-lgpl-shared-8.1/bin/${f}`, f]),
    ),
  },
  {
    name: 'JMdict English (jmdict-simplified 3.6.2+20261026175559, CC BY-SA 4.0)',
    url: 'https://github.com/scriptin/jmdict-simplified/releases/download/3.6.2%2B20261026175559/jmdict-eng-3.6.2%2B20261026175559.json.zip',
    sha256: 'c25f1594482fcb55d02f66609584367a04136af9e4cfaa15c207eb41500bc0d3',
    dir: 'vendor-cache/dict-src',
    files: { 'jmdict-eng-3.6.2.json': 'jmdict-eng.json' },
  },
  {
    name: 'Kanjium pitch accents (commit 9ebca45, CC BY-SA 4.0)',
    url: 'https://raw.githubusercontent.com/mifunetoshiro/kanjium/9ebca4589565c696e7c39e089e2e928ad65f678c/data/source_files/raw/accents.txt',
    sha256: '8bd0dd127dab32ceec94cb03ab1ba6b68858ea73421dfa1731af2f373deb4f20',
    dir: 'vendor-cache/dict-src',
    raw: true,
    files: { 'accents.txt': 'accents.txt' },
  },
];

const destDir = (dep) => (dep.dir ? join(ROOT, ...dep.dir.split('/')) : LIB);

const TAR = process.platform === 'win32' ? 'C:\\Windows\\System32\\tar.exe' : 'tar';

function sha256(path) {
  return createHash('sha256').update(readFileSync(path)).digest('hex');
}

async function download(url, dest) {
  const delays = [2000, 5000, 15000];
  for (let attempt = 0; ; attempt++) {
    try {
      const res = await fetch(url, { redirect: 'follow' });
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      writeFileSync(dest, Buffer.from(await res.arrayBuffer()));
      return;
    } catch (err) {
      if (attempt >= delays.length) throw new Error(`download failed: ${url}: ${err.message}`);
      console.log(`  retrying in ${delays[attempt] / 1000}s (${err.message})`);
      await new Promise((r) => setTimeout(r, delays[attempt]));
    }
  }
}

function extract(archive, inner, outDir) {
  const r = spawnSync(TAR, ['-x', '-f', archive, '-C', outDir, inner], { encoding: 'utf8' });
  if (r.status !== 0) throw new Error(`extract failed for ${inner} from ${archive}: ${r.stderr || r.error}`);
}

/** Compile the dictionary database from the fetched sources unless it is already built from the same sources and builder. */
function ensureDict() {
  const dictSrc = join(ROOT, 'vendor-cache', 'dict-src');
  const out = join(LIB, 'dict', 'jmdict.sqlite');
  const stampFile = join(CACHE, 'dict-stamp.txt');
  const builder = ['build.rs', 'store.rs'].map((f) => sha256(join(ROOT, 'crates', 'jpdict', 'src', f)));
  const want = JSON.stringify([DEPS.filter((d) => d.dir).map((d) => d.sha256), builder]);
  if (!force && existsSync(out) && existsSync(stampFile) && readFileSync(stampFile, 'utf8') === want) {
    console.log(`dict OK (up to date, ${(statSync(out).size / 1e6).toFixed(1)} MB)`);
    return;
  }
  mkdirSync(dirname(out), { recursive: true });
  const t0 = Date.now();
  // build-dict writes <out>.tmp and renames it on success, so a failed build never leaves a half-written database.
  const r = run('cargo', ['run', '-q', '--release', '-p', 'jpdict', '--bin', 'build-dict', '--',
    join(dictSrc, 'jmdict-eng.json'), join(dictSrc, 'accents.txt'), out]);
  if (r.status !== 0) {
    console.error('building the dictionary failed:\n' + r.out);
    process.exit(1);
  }
  writeFileSync(stampFile, want);
  console.log(`dict OK: ${out.slice(ROOT.length + 1)} ${(statSync(out).size / 1e6).toFixed(1)} MB (cargo run + build ${((Date.now() - t0) / 1000).toFixed(1)} s)`);
}

const wanted = JSON.stringify(DEPS.map((d) => d.sha256));
if (!force && existsSync(STAMP) && readFileSync(STAMP, 'utf8') === wanted &&
    DEPS.every((d) => Object.values(d.files).every((f) => existsSync(join(destDir(d), f))))) {
  console.log('deps OK (up to date)');
  ensureDict();
  process.exit(0);
}

mkdirSync(CACHE, { recursive: true });
mkdirSync(LIB, { recursive: true });
for (const dep of DEPS) {
  mkdirSync(destDir(dep), { recursive: true });
  const archive = dep.raw ? join(destDir(dep), Object.values(dep.files)[0]) : join(CACHE, decodeURIComponent(dep.url.split('/').pop()));
  if (!existsSync(archive) || sha256(archive) !== dep.sha256) {
    console.log(`downloading ${dep.name}`);
    await download(dep.url, archive);
  }
  const got = sha256(archive);
  if (got !== dep.sha256) {
    rmSync(archive, { force: true });
    console.error(`SHA-256 MISMATCH for ${dep.name}\n  expected ${dep.sha256}\n  got      ${got}`);
    process.exit(1);
  }
  if (dep.raw) {
    console.log(`  installed ${Object.values(dep.files).join(', ')} (${destDir(dep)})`);
    continue;
  }
  const tmp = join(CACHE, 'extract');
  rmSync(tmp, { recursive: true, force: true });
  mkdirSync(tmp, { recursive: true });
  for (const [inner, outName] of Object.entries(dep.files)) {
    extract(archive, inner, tmp);
    copyFileSync(join(tmp, ...inner.split('/')), join(destDir(dep), outName));
  }
  rmSync(tmp, { recursive: true, force: true });
  console.log(`  installed ${Object.values(dep.files).join(', ')}`);
}
writeFileSync(STAMP, wanted);
console.log('deps OK');
ensureDict();
