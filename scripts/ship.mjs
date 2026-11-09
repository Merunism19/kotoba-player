/** One-command ship (AGENT-WORKFLOW section 6): merge main, bump, memory check, CI, commit, push, PR, gate, merge, release; ends with `SHIP OK`. */
import { existsSync, readFileSync, writeFileSync, readdirSync } from 'node:fs';
import { resolve } from 'node:path';
import { ROOT, run, sleepSync } from './lib/util.mjs';

const argv = process.argv.slice(2);
const DRY = argv.includes('--dry-run');
const RELEASE = argv.includes('--release');
const LOCAL_GATE = argv.includes('--local-gate');
// Stop after the gate with the PR open: merging is left to the owner (their review), e.g. for stacked PRs.
const NO_MERGE = argv.includes('--no-merge');
const bumpIdx = argv.indexOf('--bump');
const BUMP = bumpIdx >= 0 ? argv[bumpIdx + 1] : 'patch';
if (!['patch', 'minor', 'none'].includes(BUMP)) die(`--bump must be patch|minor|none (got ${BUMP})`);

const P = (f) => resolve(ROOT, f);
const FILES = { commitMsg: '.ship/commit-msg.txt', prBody: '.ship/pr-body.md', notes: '.ship/release-notes.md' };
const DELAYS = [2000, 5000, 15000];

function die(msg) {
  console.error(`SHIP FAILED: ${msg}`);
  process.exit(1);
}
/** Refuse (dies; in dry-run only reports what would be refused). */
function refuse(msg) {
  if (DRY) console.log(`[dry] WOULD REFUSE: ${msg}`);
  else die(msg);
}
const say = (m) => console.log(m);
const show = (cmd, args) => [cmd, ...args].join(' ');

/** Read-only command: always runs. */
const ro = (cmd, args, opts) => run(cmd, args, opts);
/** Mutating command: printed instead of run under --dry-run. retries: back off 2s/5s/15s on failure. */
function mut(cmd, args, { retry = false, opts } = {}) {
  if (DRY) { say(`[dry] ${show(cmd, args)}`); return { status: 0, stdout: '', stderr: '', out: '' }; }
  let r = run(cmd, args, opts);
  for (let i = 0; retry && r.status !== 0 && i < DELAYS.length; i++) {
    say(`  retry ${i + 1}/3 in ${DELAYS[i] / 1000}s: ${show(cmd, args)}`);
    sleepSync(DELAYS[i]);
    r = run(cmd, args, opts);
  }
  return r;
}
const must = (r, what) => { if (r.status !== 0) die(`${what}\n${r.out.split('\n').filter(Boolean).slice(-15).join('\n')}`); return r; };
const step = (n) => say(`== ${n}`);

const semver = (v) => { const m = /^(\d+)\.(\d+)\.(\d+)$/.exec(v); if (!m) die(`bad version "${v}"`); return m.slice(1).map(Number); };
const cmp = (a, b) => { const x = semver(a), y = semver(b); for (let i = 0; i < 3; i++) if (x[i] !== y[i]) return x[i] - y[i]; return 0; };

function readVersions() {
  for (const f of ['package.json', 'src-tauri/tauri.conf.json', 'src-tauri/Cargo.toml']) if (!existsSync(P(f))) die(`missing ${f}`);
  const pkg = JSON.parse(readFileSync(P('package.json'), 'utf8')).version;
  const tauri = JSON.parse(readFileSync(P('src-tauri/tauri.conf.json'), 'utf8')).version;
  const cargo = /\[package\][^[]*?\nversion\s*=\s*"([^"]+)"/.exec(readFileSync(P('src-tauri/Cargo.toml'), 'utf8'))?.[1];
  return { pkg, tauri, cargo };
}
function writeVersions(v) {
  const sub = (file, re, to) => {
    const t = readFileSync(P(file), 'utf8');
    if (!re.test(t)) die(`no version field found in ${file}`);
    writeFileSync(P(file), t.replace(re, to)); // utf8 string write keeps LF
  };
  sub('package.json', /("version"\s*:\s*")[^"]+(")/, `$1${v}$2`);
  sub('src-tauri/tauri.conf.json', /("version"\s*:\s*")[^"]+(")/, `$1${v}$2`);
  sub('src-tauri/Cargo.toml', /(\[package\][^[]*?\nversion\s*=\s*")[^"]+(")/, `$1${v}$2`);
}

// ---- preflight
step('preflight');
for (const f of [FILES.commitMsg, FILES.prBody]) if (!existsSync(P(f))) refuse(`missing ${f}`);
if (RELEASE && !existsSync(P(FILES.notes))) refuse(`--release needs ${FILES.notes}`);
const branch = ro('git', ['rev-parse', '--abbrev-ref', 'HEAD']).stdout.trim();
let mergedAlready = false;
const prInfo = () => {
  const r = ro('gh', ['pr', 'view', branch, '--json', 'url,state,number']);
  try { return r.status === 0 ? JSON.parse(r.stdout) : null; } catch { return null; }
};
let pr = branch && branch !== 'main' ? prInfo() : null;
if (pr?.state === 'MERGED') mergedAlready = true;
if (branch === 'main' || branch === 'master') {
  if (!mergedAlready) refuse('on main — ship from a feature branch');
}

let version;
let docsOnly = false;
let ciPassed = false;

if (!mergedAlready) {
  // ---- merge origin/main
  step('merge origin/main');
  must(mut('git', ['fetch', 'origin', 'main'], { retry: true }), 'git fetch origin main failed');
  const hasMain = ro('git', ['rev-parse', '--verify', '-q', 'origin/main']).status === 0;
  if (!hasMain) say('  (no origin/main yet — skipping merge and version comparison)');
  else if (ro('git', ['merge-base', '--is-ancestor', 'origin/main', 'HEAD']).status === 0) say('  already up to date');
  else must(mut('git', ['merge', '--no-edit', 'origin/main']), 'merge origin/main failed (resolve conflicts, then re-run)');

  // ---- bump
  step('bump version');
  const v = readVersions();
  if (!(v.pkg === v.tauri && v.tauri === v.cargo)) refuse(`versions differ: package.json=${v.pkg} Cargo.toml=${v.cargo} tauri.conf.json=${v.tauri}`);
  let mainVer = null;
  if (hasMain) {
    const m = ro('git', ['show', 'origin/main:package.json']);
    try { mainVer = JSON.parse(m.stdout).version; } catch { /* main has no package.json yet */ }
  }
  version = v.pkg;
  if (mainVer && cmp(version, mainVer) < 0) refuse(`version ${version} is below main's ${mainVer}`);
  // Resumable without double-bumping, and correct for stacked branches: a version already above main
  // counts as "this branch's bump" only if this branch recorded it (a stacked branch inherits its
  // parent's bump and still needs its own).
  const bumpLog = P('.ship/bumps.json');
  let bumps = {};
  try { bumps = JSON.parse(readFileSync(bumpLog, 'utf8')); } catch { /* first ship on this machine */ }
  const bumpedHere = bumps[branch] === version;
  const needsBump = BUMP !== 'none' && (!mainVer || cmp(version, mainVer) === 0 || !bumpedHere);
  if (needsBump) {
    const [a, b, c] = semver(version);
    version = BUMP === 'minor' ? `${a}.${b + 1}.0` : `${a}.${b}.${c + 1}`;
    if (DRY) say(`[dry] bump ${v.pkg} -> ${version} in package.json, src-tauri/Cargo.toml, src-tauri/tauri.conf.json`);
    else {
      writeVersions(version);
      must(mut('cargo', ['update', '--workspace', '--offline']), 'cargo update --workspace failed (Cargo.lock must follow the new version)');
      writeFileSync(bumpLog, JSON.stringify({ ...bumps, [branch]: version }, null, 2));
    }
  } else if (mainVer && cmp(version, mainVer) === 0) {
    // A docs-only change (every changed file is docs/ or Markdown) ships without a new version.
    const files = new Set();
    for (const l of ro('git', ['diff', '--name-only', 'origin/main...HEAD']).stdout.split('\n')) if (l) files.add(l);
    for (const l of ro('git', ['status', '--porcelain', '-uall']).stdout.split('\n')) if (l.length > 3) files.add(l.slice(3).replace(/^"|"$/g, ''));
    const docs = files.size > 0 && [...files].every((f) => f.startsWith('docs/') || f.endsWith('.md'));
    if (!docs) refuse(`--bump none but version ${version} equals main's (only docs-only changes may keep the version)`);
    say(`  docs-only change: keeping v${version}`);
  }
  else say(`  version ${version} already ahead of main (${mainVer ?? 'n/a'})`);

  // ---- memory check
  step('memory check');
  const handoff = existsSync(P('docs/HANDOFF.md')) ? readFileSync(P('docs/HANDOFF.md'), 'utf8') : '';
  const sec = handoff.split(/^## /m).find((s) => s.startsWith('Current state')) ?? '';
  // The section must START with the version being shipped (a mention like "on top of v0.2.0" doesn't count).
  const firstLine = sec.split(/\r?\n/).slice(1).find((l) => l.trim() !== '') ?? '';
  if (!firstLine.trim().startsWith(`v${version}`)) refuse(`docs/HANDOFF.md "## Current state" must start with v${version} (found: "${firstLine.trim().slice(0, 40)}")`);
  else say(`  HANDOFF names v${version}`);

  // ---- codemap
  step('codemap');
  must(mut('node', ['scripts/codemap.mjs']), 'codemap failed');

  // ---- CI
  step('local CI');
  const changed = new Set();
  if (hasMain) for (const l of ro('git', ['diff', '--name-only', 'origin/main...HEAD']).stdout.split('\n')) if (l) changed.add(l);
  for (const l of ro('git', ['status', '--porcelain', '-uall']).stdout.split('\n')) if (l.length > 3) changed.add(l.slice(3).replace(/^"|"$/g, ''));
  docsOnly = changed.size > 0 && [...changed].every((f) => f.startsWith('docs/') || f.endsWith('.md'));
  const ciArgs = ['scripts/ci.mjs', ...(docsOnly ? ['--docs-only'] : [])];
  if (DRY) { say(`[dry] node ${ciArgs.join(' ')}  (${changed.size} changed files, docsOnly=${docsOnly})`); }
  else {
    const r = run('node', ciArgs);
    say(r.out.trimEnd());
    if (!r.stdout.trimEnd().endsWith('CI OK')) die('local CI did not report CI OK');
    ciPassed = true;
  }

  // ---- commit
  step('commit');
  must(mut('git', ['add', '-A']), 'git add failed');
  const staged = DRY ? true : ro('git', ['diff', '--cached', '--quiet']).status !== 0;
  if (staged) must(mut('git', ['commit', '-F', FILES.commitMsg]), 'git commit failed');
  else say('  nothing staged — skipping');

  // ---- push
  step('push');
  const upstream = ro('git', ['rev-parse', '-q', '--verify', '@{u}']).stdout.trim();
  const head = ro('git', ['rev-parse', 'HEAD']).stdout.trim();
  if (!DRY && upstream && upstream === head) say('  already pushed');
  else must(mut('git', ['push', '-u', 'origin', 'HEAD'], { retry: true }), 'git push failed');

  // ---- PR
  step('pull request');
  pr = prInfo();
  let bodyFile = FILES.prBody;
  if (LOCAL_GATE) {
    bodyFile = '.ship/pr-body.final.md';
    const body = readFileSync(P(FILES.prBody), 'utf8').replace(/\n*$/, '\n');
    const line = '\nGate: local CI (`node scripts/ci.mjs`) — hosted checks were not used for this merge.\n';
    if (DRY) say(`[dry] write ${bodyFile} (= ${FILES.prBody} + local-gate line)`);
    else writeFileSync(P(bodyFile), body + line);
  }
  if (pr && pr.state === 'OPEN') {
    say(`  exists: ${pr.url}`);
    // An existing PR must also say it was gated locally (AGENT-WORKFLOW section 9).
    if (LOCAL_GATE) {
      must(mut('gh', ['pr', 'edit', String(pr.number), '--body-file', bodyFile], { retry: true }), 'gh pr edit failed');
    }
  } else {
    const title = readFileSync(P(FILES.commitMsg), 'utf8').split('\n')[0].trim();
    must(mut('gh', ['pr', 'create', '--base', 'main', '--head', branch, '--title', title, '--body-file', bodyFile], { retry: true }), 'gh pr create failed');
    pr = DRY ? { url: '<PR url>', number: '<n>', state: 'OPEN' } : prInfo();
    if (!pr) die('PR was created but cannot be read back');
  }

  // ---- gate
  step('gate');
  if (LOCAL_GATE) {
    if (DRY) say('[dry] gate = local CI result (--local-gate)');
    else if (ciPassed) say('  local CI passed (--local-gate)');
    else die('local gate requested but local CI did not pass');
  } else if (DRY) {
    say(`[dry] gh pr checks ${pr.number} --watch --fail-fast`);
    say(`[dry] gh pr checks ${pr.number} --json name,bucket   (result read from JSON, never from the watch exit status)`);
  } else {
    const num = String(pr.number);
    ro('gh', ['pr', 'checks', num, '--watch', '--fail-fast']); // only waits; the verdict is read below
    let verdict = 'none';
    for (let attempt = 0; attempt < 4; attempt++) {
      const r = ro('gh', ['pr', 'checks', num, '--json', 'name,bucket']);
      let checks = null;
      try { checks = JSON.parse(r.stdout); } catch { /* no checks reported */ }
      if (!checks || checks.length === 0) {
        if (docsOnly) { verdict = 'none-docs'; break; }
        verdict = 'none';
      } else if (checks.some((c) => c.bucket === 'fail' || c.bucket === 'cancel')) {
        die('hosted checks failed:\n' + checks.filter((c) => c.bucket !== 'pass' && c.bucket !== 'skipping').map((c) => `  ${c.name}: ${c.bucket}`).join('\n'));
      } else if (checks.every((c) => c.bucket === 'pass' || c.bucket === 'skipping')) { verdict = 'pass'; break; }
      else verdict = 'pending';
      sleepSync(10000);
    }
    if (verdict === 'pass') say('  hosted checks passed');
    else if (verdict === 'none-docs') say('  docs-only change: hosted CI skipped by paths-ignore');
    else die(`gate not satisfied (checks ${verdict}); use --local-gate if hosted CI is unavailable`);
  }

  if (NO_MERGE) {
    say(`SHIP READY: v${readVersions().pkg} ${pr.url} (gate passed; merge left to the owner)`);
    process.exit(0);
  }
  // ---- merge
  step('merge');
  must(mut('gh', ['pr', 'merge', String(pr.number), '--squash', '--delete-branch'], { retry: true }), 'gh pr merge failed');
  if (!DRY && prInfo()?.state !== 'MERGED') die('PR is not MERGED after gh pr merge');
}

// ---- back to main
step('checkout main + pull');
must(mut('git', ['checkout', 'main']), 'git checkout main failed (is main checked out in another worktree?)');
must(mut('git', ['pull', '--ff-only', 'origin', 'main'], { retry: true }), 'git pull failed');
if (!version) version = DRY ? '<version>' : readVersions().pkg;
pr = pr ?? prInfo();

// ---- release
if (RELEASE) {
  step('release');
  const tag = `v${version}`;
  const bundleDir = P('src-tauri/target/release/bundle/nsis');
  const installers = () => (existsSync(bundleDir) ? readdirSync(bundleDir).filter((f) => f.endsWith('-setup.exe') && f.includes(version)) : []);
  if (!DRY && installers().length) say('  installer already built');
  else must(mut('pnpm', ['tauri', 'build']), 'pnpm tauri build failed');
  const view = () => {
    const r = ro('gh', ['release', 'view', tag, '--json', 'assets,url']);
    try { return r.status === 0 ? JSON.parse(r.stdout) : null; } catch { return null; }
  };
  let rel = view();
  if (!rel) must(mut('gh', ['release', 'create', tag, '--title', tag, '--notes-file', FILES.notes, '--target', 'main'], { retry: true }), 'gh release create failed');
  else say(`  release ${tag} exists — uploading missing assets only`);
  const have = new Set((rel?.assets ?? []).map((a) => a.name));
  const files = DRY ? [`<${tag} installer>`] : installers();
  if (!DRY && files.length === 0) die('no NSIS installer found after build');
  for (const f of files) {
    if (have.has(f)) { say(`  asset ${f} present`); continue; }
    must(mut('gh', ['release', 'upload', tag, DRY ? f : resolve(bundleDir, f)], { retry: true }), `upload of ${f} failed`);
  }
  if (!DRY) {
    rel = view();
    const names = new Set((rel?.assets ?? []).map((a) => a.name));
    for (const f of files) if (!names.has(f)) die(`asset ${f} missing from release after upload`);
  }
}

if (DRY) say('DRY RUN COMPLETE (nothing was changed; no SHIP OK)');
else say(`SHIP OK: v${version} ${pr?.url ?? ''}`.trimEnd());
