// Downloads the pinned third-party binaries (libmpv, libmpv-wrapper, ffmpeg/ffprobe), checks their SHA-256 and installs them into src-tauri/lib/.
//
// Usage: node scripts/fetch-deps.mjs [--force]
// Archives are cached in vendor-cache/ (gitignored). Extraction uses Windows' bundled bsdtar
// (C:\Windows\System32\tar.exe reads .7z and .zip). To upgrade a dependency: change its entry
// below (url + sha256 from the GitHub release's digest), run with --force, test, commit.
import { createHash } from 'node:crypto';
import { spawnSync } from 'node:child_process';
import { copyFileSync, existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const CACHE = join(ROOT, 'vendor-cache');
const LIB = join(ROOT, 'src-tauri', 'lib');
const STAMP = join(CACHE, 'deps-stamp.json'); // not in lib/: everything there is bundled
const force = process.argv.includes('--force');

/** Pinned dependencies. `files` maps the path inside the archive to the installed file name. */
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
    name: 'FFmpeg n8.1.3 (BtbN, LGPL, win64 static)',
    url: 'https://github.com/BtbN/FFmpeg-Builds/releases/download/autobuild-2026-10-30-13-01/ffmpeg-n8.1.3-27-g8dca1586f4-win64-lgpl-8.1.zip',
    sha256: 'bf346925ea085c262d15a5ceacd521ccbde52f4e3fc6a97f1bf273685fc1a539',
    files: {
      'ffmpeg-n8.1.3-27-g8dca1586f4-win64-lgpl-8.1/bin/ffmpeg.exe': 'ffmpeg.exe',
      'ffmpeg-n8.1.3-27-g8dca1586f4-win64-lgpl-8.1/bin/ffprobe.exe': 'ffprobe.exe',
    },
  },
];

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

const wanted = JSON.stringify(DEPS.map((d) => d.sha256));
if (!force && existsSync(STAMP) && readFileSync(STAMP, 'utf8') === wanted &&
    DEPS.every((d) => Object.values(d.files).every((f) => existsSync(join(LIB, f))))) {
  console.log('deps OK (up to date)');
  process.exit(0);
}

mkdirSync(CACHE, { recursive: true });
mkdirSync(LIB, { recursive: true });
for (const dep of DEPS) {
  const archive = join(CACHE, dep.url.split('/').pop());
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
  const tmp = join(CACHE, 'extract');
  rmSync(tmp, { recursive: true, force: true });
  mkdirSync(tmp, { recursive: true });
  for (const [inner, outName] of Object.entries(dep.files)) {
    extract(archive, inner, tmp);
    copyFileSync(join(tmp, ...inner.split('/')), join(LIB, outName));
  }
  rmSync(tmp, { recursive: true, force: true });
  console.log(`  installed ${Object.values(dep.files).join(', ')}`);
}
writeFileSync(STAMP, wanted);
console.log('deps OK');
