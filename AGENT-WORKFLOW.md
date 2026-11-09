# Agent workflow for building software (plug and play)

A general method for one orchestrating agent plus helper agents to build, verify and ship an app
for a long time with little supervision. It is **project-agnostic**: copy this file into any repo
(e.g. `docs/AGENT-WORKFLOW.md`) and point the project's agent instructions at it. Project-specific
details (commands, rules, current state) live in that project's own handoff and workflow files;
this file holds what generalizes.

It also **improves itself**: every agent that uses it follows the improvement protocol in
section 12 and leaves the file better than it found it. Version and history are at the end.

---

## 0. Adopting it in a new project (do this once)

1. Copy this file into the repo. Create, if missing:
   - `docs/HANDOFF.md` — the project's memory: what it is, the owner and their rules, how to
     build/test/ship, gotchas, current state, next steps, a short status log.
   - `docs/WORKFLOW.md` — the project-specific instance of this method (its scripts, gates,
     commands) with its own changelog.
   - `docs/agents/BRIEF.md` — the rules every helper follows (section 4).
   - The agent entry file (`CLAUDE.md`, `AGENTS.md`…) saying: read HANDOFF, then WORKFLOW, then
     this file; keep them current in the same commit as any change.
2. Write the hard rules down before the first feature (security, licences, privacy, "never do X
   on the user's device"). Rules only in someone's head get broken.
3. Script the loop's repeated steps early (section 6). The second time you type a sequence,
   script it.
4. Add the cheap guards from section 8 that fit the stack.

## 1. Roles

- **Orchestrator** (the strongest model available): picks work, writes briefs, reviews, merges,
  ships, keeps the docs true, reports to the owner. Reads short reports, never full transcripts.
- **Helpers** (a cheaper model): write the code for one well-scoped piece, run the checks, reply
  in a few lines. Good briefs make them nearly as reliable as the orchestrator at a fraction of
  the cost.
- **Owner** (the human): decides product and anything involving their accounts, money, publishing
  or devices. Often away. Their standing instructions are rules (record them; section 10).

## 2. The loop (one piece of work)

1. **Pick** the next item: the owner's goal first, then the design/spec, then the handoff's next
   steps, then what users of similar apps ask for (a quick search beats guessing). Check with one
   grep whether it already exists — several "missing" features turn out to exist — and whether the
   pinned version of a dependency already does it (its `--help`, changelog, bundled tools).
2. **Measure first** when it's about speed, size or cost (list the build output, time the slow
   path). Measurement turns vague tasks into one-line fixes. **Measure on the target device**: a
   speed-up proven on the build machine can be worth nothing there (or break it); time each candidate
   on the device before building on it, and repeat runs back to back because devices are noisy.
3. **Branch** from the main branch (or stack on an unmerged branch only when the work depends on
   it; keep stacks short — three is the most worth carrying). Merge a stack bottom-up with merge commits,
   not squash: a squashed parent leaves the next branch's copies of the same commits to conflict.
4. **Brief** one helper (section 4). One writer per checkout at a time; to run helpers in parallel
   (or alongside your own edits), give each its own git worktree and bring its files over after review.
   A new worktree may be cut from the default branch, not your working branch: commit what the helper
   needs on a branch it will start from, name the expected base commit in the brief, and have it check
   `git log -1` before its first edit. Contract first: commit the shared types/interfaces (and skeleton
   modules) the helpers build against, then cut every worktree from that commit. Lockfile conflicts when
   merging parallel branches: keep one side and let the package tool re-add the missing entries.
5. **While it works**, do only things that don't touch its files or the build lock: plan the next
   brief, write release notes, update docs.
6. **Review** (section 5).
7. **Ship** with one command (section 6): bump version, commit, PR, gate, merge, release.
8. **Report** to the owner in plain words: what's done, where to get it, what isn't verified,
   what the workflow learned.
9. **Improve the workflow** if anything in this round cost time, tokens or a retry (section 12).

## 3. Planning and scope

- **Start delegating from the first independent piece**, even on day one of a new project. Writing a
  tightly coupled core yourself is fine; the screens, tests and docs around it are helper work.
- Prefer finishing over starting: a feature isn't done until it's merged, shipped and noted.
- Split big work so each piece ends in something shippable. Big *mechanical* jobs (renames,
  moving strings, migrations across many files) go in briefs of about 15 files: a helper's cost
  grows faster than its work, because every step re-reads its whole history.
- Decide small product questions yourself, write the decision down where the next agent will see
  it, and move on. Ask the owner only about their accounts, money, publishing, devices or genuine
  product direction — once, with clear options — and keep working on other things meanwhile.
- **Training a model for a feature** (distilling a big local model into a small one): grade every
  example with strict, automatic rules; *repair* failing parts with the big model instead of
  discarding them (pass rate 11 % → 47 %); read samples yourself at every stage (metrics missed that the
  generated requests were diary entries); check that **every category reached the final dataset** —
  a safety category had zero examples after filtering because the grader demanded a label the
  teacher never wrote; and evaluate the model the way the product will run it (chunked, repaired),
  with the same prompt strings in the app (parity tests), not a single raw call.
- **Structural rules belong in decoding, not only in repair:** forbidding a repeated line opening while the
  model writes beat after-the-fact repair by a wide margin (long sets 32 → 54 of 82; overall 98 → 136 of 160).
- **For high-risk requests, put a deterministic layer in front of a small model.** A keyword metric said 8/8
  safe while reading the lines showed reinforcement of the harm; route those categories to vetted, pre-written
  output instead, and test the router with real phrasings and near misses.

## 4. Writing a brief

A project keeps the *how* in `docs/agents/BRIEF.md` (written once, pointed to by every brief):
hard rules; style/lint limits; how to edit files safely on this OS; the exact verify command,
to be run *after the last edit* (helpers tidy up after verifying and report stale results);
"don't commit, push or switch branches"; "stop every background process and wait for background commands before replying";
"write a change list file" (section 5); "claim only what you actually ran"; where to log status.

Each brief then says only **what**:

- the goal in one or two sentences, with the spec's own words when there is one;
- numbered parts, each with acceptance criteria;
- exact paths to read first and code to reuse (a generated code map makes this cheap — section 7);
- for a follow-up on code you just reviewed: the exact functions and lines to change — you already
  hold that context, the helper would spend most of its budget rebuilding it;
- decisions already made (defaults, limits, security rules for this feature);
- tests to write;
- the reply format and length (6–12 lines: what was built, files + change-list check, what exactly
  ran, what needs a real device).

Vague briefs are the most expensive thing in this workflow: the helper explores instead of
building. Complete briefs are cheap.

## 5. Reviewing what comes back

1. Run the change-list check: the helper's list of files vs. what version control sees.
2. Grep any claim about build config, CI, permissions or security — one line each. Helpers
   sometimes report intentions as facts. The same goes for your own claims from upstream docs: check a
   dependency feature in the artifact you ship (its jar/AAR, the APK), not on the project's main branch.
3. Read only the riskiest function (a security decision, a migration, an exported entry point),
   not the whole diff.
4. Check what ships: new permissions, new dependencies (licence), new exported components.
5. If something's off, send the **same** helper a short follow-up — it still has its context.
6. If it left background work running, stop it before committing.

**Hunting bugs when nobody can use the product (owner away, device locked):** run a review loop per risk
area, highest stakes first (user data written or deleted, backup/restore, crash paths, concurrency):
a read-only reviewer helper gets one area and must quote the lines and give a concrete repro for every
finding; a separate fixer helper fixes the verified ones with tests; the orchestrator reads only the
riskiest new function itself (lifecycles, locks, delete paths), then runs the device or integration
tests of every module the fix touched before shipping. Reviewers find real bugs; fixers introduce new
ones in exactly the code that is hard to unit-test (service lifecycles, provider quirks), so the last two
steps are not optional.

## 6. Shipping in one command

Script the end of every task once: bump version → regenerate generated docs → run the cheap guards
→ commit (message from a file) → push → open the PR (body from a file) → gate → merge → update the
main branch → build, check and upload the release → print one final success line.

Make the script:
- **resumable** — skip steps already done (commit exists, PR exists, already merged);
- **parallel-safe** — merge the main branch in before the gate (other agents ship too) and refuse a
  version number that isn't above the one now on main;
- **retrying** — retry every network/API call a few times with backoff; finish half-made
  releases instead of creating them again;
- **honest** — end with a single unmistakable line (`SHIP OK: …`); its absence means failure.
  Never trust the exit status of a piped command.
- **proportionate** — a docs-only change skips the build and runs only the text checks;
- **memory-enforcing** — refuse to ship a release the handoff's current-state section doesn't
  name, so the project memory can never fall behind the code.

Text for commits, PRs and notes always goes in files, never in quoted command-line arguments.

## 7. Making helpers cheap

- **Generated code map:** a script lists every source file with its first doc-comment sentence;
  the ship script regenerates it. Helpers find files without searching (observed: about half the
  cost of a comparable task).
- **Shared rules file** (section 4) instead of repeating rules in every brief (briefs about half
  as long).
- **Quiet builds:** run tools in quiet mode and filter to errors; passing builds should print
  nothing.
- **Small diagnosed fixes stay with the orchestrator.** When you already hold the cause and the fix is a
  few lines, make it yourself; briefing and reviewing a helper costs more than the edit.
- **Parse test reports with a script**, never by eye: report files mix totals and per-suite lines.
- **Never read transcripts**, only reports. Unpack big inputs (designs, specs, archives) once and
  keep a summary in the handoff.
- **Keep the handoff short:** move old log lines and history to a separate file once the log
  passes about 8 lines; the handoff is read by every new agent.

## 8. Guards: turn repeated mistakes into checks

When the same kind of mistake happens twice, add a check that runs in local CI and CI, and prove
it by planting the mistake once. Cheap guards that paid off in practice:

- **Probes of the running app print their own inputs first** (arguments, paths, ports it handed the
  app) and wait until the previous run's processes are gone. A failing probe is as often a harness bug
  (a quoting slip, a leftover child process answering) as an app bug, and its first line should say which.
- **See the UI without the device:** headless screenshot tests (render real screens to images on the
  build machine) let the orchestrator look at every screen before shipping. Previews in another renderer
  (an HTML page of the same icons) can look right while the real renderer draws them wrong.

- **Text encoding:** every source file valid UTF-8, with no stray control characters or CR bytes
  (editors, shells and scripted edits on some OSes rewrite bytes silently; on Windows a script's
  plain text write turns LF into CRLF). Prefer the agent's file-edit tool to scripted replacements,
  and never script edits of code that contains escapes (backslash-n, backslash-r-backslash-n, regexes):
  one language's string layer silently decodes them for the other.
- **Hard rules tested at the real entry point,** not only the inner API: an invariant can hold in the
  core and still break where outside input is loaded (a file read that translates newlines).
- **Resource syntax** (e.g. unescaped quotes in UI string files) — seconds instead of a failed
  multi-minute build.
- **Architecture boundaries:** feature modules may only depend on shared layers, with a short,
  commented allow-list.
- **Licences:** every dependency matched against an allow-list of acceptable licences.
- **Hard-coded UI text** in translated code.
- **Data stores are registered:** every settings/storage file goes through one registry with an
  explicit backup/export decision (and a reason when excluded); a check forbids opening stores any
  other way, and a test matches the registry to the backup allow-list. Features added one by one
  otherwise forget backups silently.
- **Permissions / capabilities** that must never appear in a release (check every build output).
- **Fuzz tests** for every parser of outside input: seeded, mutated inputs, time budgets per batch
  to catch quadratic blow-ups, the failing seed printed. They find real bugs (overflows, unescaped
  exceptions, deep-nesting crashes) cheaply.
- **Test against another tool's real output, saved as a fixture:** parsers of its output, and any
  deny-list of its flags or commands (a test walks the tool's own alias list). Hand-written samples
  match what you expected, not what the tool prints.
- **Framework-integration invariants** get a test of their own: e.g. a reflection test that the
  framework callback really overrides every method the app relies on. Language shortcuts can
  silently replace your code with the framework's defaults, and no feature test notices.

**Unit tests and green CI are not enough: run the real build on a real device after every few
releases.** Ten minutes of using the installed app (and reading the system crash log) found two
shipped regressions that every automated check had passed: a crash in a separate process whose
tests used a different app class, and a callback whose methods silently fell back to defaults.
When the device is available, install over the user's data, exercise each new feature once, and
grep the log for crashes and refusals.

## 9. Gates, CI and money

- **Merge only through a guard** that refuses unless every check passed. Read the wait's result
  before acting on it.
- **Keep a local CI script** that runs everything CI runs. When hosted CI can't run (outage,
  billing, quotas), gate on local CI, say so on the PR, and keep going.
- **CI minutes cost money.** Run on pull requests only (not again after merge), skip docs-only
  changes, cancel superseded runs, cache expensive builds keyed on *every* file the cached step reads
  (a config file the build script sources counts; a missed one silently serves a stale build), and keep
  scheduled jobs (fuzzing, nightly) short and infrequent. One forgotten nightly hour can use a
  month's free quota.
- Combine small related changes into one PR to save CI runs.
- **Rented compute (GPU pods, cloud boxes) bills by the hour until something ends it.** Prove the
  off-switch works before the long job starts (the job refuses to run when its stop command can't see
  the machine), and make a safety timer fire only at its deadline: loop on the clock and ignore
  interrupts, never "sleep N, then stop" (an interrupted sleep runs the stop at once). Measure
  throughput in the first minutes and re-pick the hardware then; the bottleneck may be memory, not
  compute. Trust the machine's own output over the provider's dashboard, and never take a restarting
  action (resizing a disk, changing an image) to "unstick" something you haven't confirmed is stuck.

## 10. Working with the owner

- Record every standing instruction as a rule (in the project's handoff and, if available, the
  agent's persistent memory) with the reason, so later sessions follow it.
- **Never wait idle on the owner.** If something truly needs them, ask once (notify their device
  if they may be away), then continue with work that doesn't depend on the answer. Don't re-ask or
  re-check every turn; idle loops cost tokens and add nothing.
- Don't do hard-to-reverse or outward-facing things on inference (publishing, making things
  public, money, account settings, someone else's device). Offer them as options.
- Keep a single, ordered **device test plan** for anything that needs the owner's hardware; every
  PR adds its checks there, not only in the PR body.
- Plain, short reports. Lead with what's done and where to get it; say what isn't verified.

## 11. Recovery

- **Usage/rate limit mid-task:** when it resets, tell the same helper to continue; check what it
  already wrote first.
- **Hosted service errors (5xx):** the ship script retries; if it stops, re-run it — it resumes.
- **A gate fails:** read only the failing lines, fix on the same branch, re-run the ship script.
- **About to run out of budget:** commit and push a work-in-progress note in the handoff saying
  what's half-done and how to resume.

## 12. Self-improvement protocol (every agent, every session)

This file is meant to get better with use. Apply this loop at the end of every task or round:

1. **Notice.** Anything that cost a retry, a failed build, a re-read, a manual step done twice, a
   long wait, or a surprise is a candidate.
2. **Fix the cause, not the instance.** Prefer, in this order: a script or guard that makes the
   mistake impossible or obvious → a rule in the project's BRIEF/WORKFLOW → a sentence here.
   Prove guards by planting the mistake once.
3. **Generalize.** If the lesson would help in another project, add it to this file in the right
   section, in general terms (no project names, paths or tools that only one project has). If
   it's project-specific, put it in the project's own workflow file instead.
4. **Keep it lean.** Edit existing guidance rather than appending near-duplicates. Merge or
   delete rules that turned out wrong or obsolete. Every rule should earn its line: if no agent
   would behave differently because of it, cut it.
5. **Record evidence.** Add a changelog line below: date, what changed, and the observation that
   justified it (a number when there is one: minutes saved, tokens, failures avoided). Rules
   without evidence get removed in the next review.
6. **Review periodically.** Every ~10 changelog entries, re-read the whole file top to bottom,
   consolidate, and bump the version.
7. **Never weaken safety** to save time: hard rules, review of security-relevant changes, gates
   and "don't do outward-facing things on inference" stay, however tempting.

If you are an agent reading this in another project: you are expected to improve this file the
same way. Copy improvements that generalize back to the canonical copy if you know where it is.

---

## Version and history

**Version 1.0** (2026-11-04). Distilled from building an Android app over ~90 PRs with an Opus
orchestrator and Sonnet helpers, mostly without the owner present.

Changelog (newest last; each line: what changed — evidence):
- 2026-11-04 v1.0: First general version. Evidence behind the main rules:
  shared brief file → briefs ~50 % shorter; generated code map → a comparable helper task cost
  ~100k tokens / 4 min instead of 150–250k / 8–15 min; one-command resumable, retrying ship script
  with a final success line → survived a day of repeated hosted-service 5xx errors and caught a
  failed push that a piped exit status hid; encoding and resource guards → caught three
  separate byte/escape slips in seconds; merge guard → added after a merge on a red CI run;
  CI-minute rules → an hour-long nightly job had used most of a monthly quota and stopped CI;
  briefs of ~15 files for mechanical work → one 93-file brief cost ~540k tokens; "never wait on
  the owner" → four PRs sat idle waiting on a billing decision.
- 2026-11-04: Briefs require the verify run *after the last edit* — a helper reported clean, then
  the ship gate failed on a detekt finding from an edit made after its last verify.
- 2026-11-04: Guard "data stores are registered" — backups had silently missed settings that a
  dozen features stored in their own files; a registry + check + test makes forgetting impossible.
- 2026-11-04: Shipping script is proportionate and memory-enforcing — a docs-only PR spent a full
  ~10 min build, and the handoff's current state was found 26 PRs stale.
- 2026-11-04: For a small follow-up on code you just reviewed, put the exact functions/lines in the
  brief — a helper given only file names spent an hour re-reading before its first edit. Helpers
  must not report with background commands still running (they wake and repeat the report).
- 2026-11-04: Cache keys cover every input — a build's component list lived in a sourced file the CI
  cache key didn't hash, so a codec fix would have shipped with the old cached build. Test reports are
  parsed by a script — reading one by eye counted the total as a suite and doubled the pass count.
  Small diagnosed fixes stay with the orchestrator — five fixes of 1–20 lines each in one round.
  Ship script is parallel-safe — another agent's hotfix landed during a round: the PR couldn't merge
  and both releases had taken the same version code.
- 2026-11-04: Real-device check after releases + framework-invariant guards — on-device use found two
  regressions every unit test and CI gate had passed (isolated-process crash; Kotlin `by` delegation
  silently dropping Java default methods, so every playback command was refused for two releases).
- 2026-11-05: Look at the real renderer's output — an HTML preview of the app's icons looked right while the
  app drew several of them wrong (its path parser differs); headless screenshot tests showed it at once.
  Also review meaning, not only rendering: the same icon was reused for unrelated features.
- 2026-11-05: Model-training rules (section 3) — evidence: strict grading + repair took a teacher's usable
  answers from 11 % to 47 %; reading samples caught wrong-genre training requests that no metric flagged;
  the redirect (safety) category had 0 of 175 examples left after filtering until checked by category.
  When a gate is red, read which step failed before theorising (a hidden glyph-check failure was
  blamed on memory pressure three times); ship scripts must print the failing step.
- 2026-11-05: Decoding-time constraints and a deterministic high-risk router (section 3) — evidence: opening
  block 98 → 136/160 passes; a harm-word check reported 8/8 clean on explicit self-harm/minor/control requests
  whose lines were unsafe in substance ("I am gone and I am safe").
- 2026-11-05: A platform difference shipped as a crash in four releases: code that compiles for a device but is
  only ever tested on the build machine's runtime can diverge (here a regex flag Android rejects). For every
  new platform-sensitive component add one tiny on-device test, and a static check for the known traps.
- 2026-11-05: A graph rewrite that ran 6x fewer nodes and 25-40 % faster on the PC gave nothing on the phone
  (compute-bound there) and its main model ran out of memory; thread counts measured on the phone gave the real win
  (fast model 1 thread, 3/1/3 = 3.6x real time vs 4.3-17x). Measure every candidate on the target device first;
  phone timings swing 2-5x with memory pressure, so compare configurations back to back in one run.
- 2026-11-06: Review loop per risk area (section 5). In one unattended night: 8 read-only reviews (security,
  release-build/R8, accessibility, playback, Studio core, files the app writes, backup/restore) found ~40
  verified bugs, incl. a restore that could delete the library, edits that could hide a song and drop its
  playlist rows, a blank line breaking a whole feature; all fixed in ~12 PRs. The orchestrator's own read
  of the riskiest new function caught two fixer-introduced races (a stopSelf start-id race that would
  cancel a just-queued job, a main-thread lock wait that would freeze the UI), and device tests run after
  a release caught a fixer regression (a positional cursor read) that every JVM test had passed.
- 2026-11-06: Rented-compute rule (section 9). First real GPU training run: the pod's built-in API key
  could not stop the pod (the self-stop would have failed silently, billing on); a Ctrl-C fired the
  "sleep then stop" watchdog at once; a disk resize meant to "unstick" a download that had in fact
  finished restarted the container (~30 min of setup again); a stale dashboard 0% GPU figure was a false
  alarm. FP8 weights doubled the KV cache (1.6x throughput) and two bigger-memory GPUs gave ~2.5x a
  single one: hardware re-picked after 30 minutes of measuring finished all 39,240 answers in one night.
- 2026-11-07: Delegate from the first independent piece; verify dependency features in the shipped artifact —
  an orchestrator wrote a new app's whole core itself until the owner asked why no helpers were used (the two
  helpers then did UI + first build and 79 tests for ~215k tokens); a library feature read on its main branch
  had to be re-checked in the released AAR before relying on it.
- 2026-11-07: Parallel helpers in worktrees; CR guard; hard rules tested at the entry point — on a new
  project's first day two helpers (CI/ship scripts ~103k tokens, test suite ~102k) ran in their own
  worktrees while the orchestrator wrote the core, with no conflicts; scripted edits wrote CRLF into 5
  files (the encoding guard caught it) and a mangled escape shipped in a doc; a byte-for-byte
  system-prompt test passed on the core while the CLI's file read translated CRLF, caught only by review.
- 2026-11-07: Check the dependency's pinned version before building; real-output fixtures for parsers and
  deny-lists — the inference server's built-in memory fitting already did a whole planned component
  (found in its `--help`, confirmed by a 3-model dry run), so the planner shrank to setting one value; a
  help-text parser written from expectations missed that flag (two spaces after an alias comma) and
  only a run against the real binary showed it; a helper's deny-list for user-supplied server flags
  missed 15+ aliases incl. every model-download flag, caught by reading the real alias list in review.
- 2026-11-07: For a gate over a shell, "allow only provably simple" beats "allow unless detected"; script
  merges must check the exit status — two adversarial rounds on a PowerShell gate: round 1 found ~12
  bypasses (backslashes eaten by a POSIX tokenizer, code inside arguments of "safe" commands), round 2
  ~8 more one layer down (comma arrays, wildcards into junctions, `-MemberName Delete`); each deny-list
  patch invited the next bypass, while switching auto mode to the read-only allowlist closed the whole
  class at once. Separately, two helper merges were silently skipped because a regenerated file was
  dirty and the output filter hid git's refusal; four per-risk-area reviews of a finished app found
  ~45 real bugs that ~600 green tests had missed (non-atomic writes, U+2028 splitting JSONL, timeouts
  blocking on grandchildren's pipes).
- 2026-11-08: Worktree base commit in the brief; probes print their inputs and wait for old processes —
  a helper's worktree was cut from main (docs only) instead of the scaffold branch, so it wrote a stand-in
  package.json and tested Rust/tsc steps against throwaway crates (~120k tokens; merged by hand); about 8
  reruns of a desktop-app probe chased "load failures" that were a `\\$` shell-quoting slip in the path
  and a leftover WebView2 process reused by the next launch. Also seen again: a piped tool check printed
  blanks instead of "missing", and the PATH had a 2013 ffmpeg shadowing a current one — pin and bundle
  external binaries rather than trusting PATH.
- 2026-11-08: Contract-first parallel helpers — a committed UI↔core contract (types + skeleton crates) let three
  helpers (parser ~124k tokens, tokenizer ~162k, UI + screenshot harness ~203k) work in worktrees cut from that
  commit while the orchestrator wrote the core against the same contract; all three merged with only a
  Cargo.lock conflict (kept ours, cargo re-added entries) and the real app worked on the first probe.
- 2026-11-08: Stacked PRs and escapes. A ship script's "version already above main = already bumped" rule
  skipped the bump on a stacked branch (it inherited the parent's bump), and its memory check accepted a
  version merely mentioned in the handoff ("on top of v0.2.0"); fixed with a per-branch bump record and a
  "current state starts with the version" rule, proven by planting. Separately, scripted (Python heredoc)
  edits turned backslash escapes inside JS/Rust strings into real line breaks three times in one session (a
  syntax error, a test hanging on a header loop, and this very changelog line) — code or text with escapes
  is now edited only with the edit tool.
- 2026-11-08: Review loop on a young app paid off at once: a read-only security reviewer (~104k tokens) found that
  a media plugin's default permission let any WebView script run arbitrary player commands (incl. process
  launch) — fixed by an allowlisting Rust layer, a capability guard proven by planting, a strict CSP and a
  sandboxed preview iframe; a data/crash reviewer (~149k) found colliding temp files, a cache key that would
  overwrite earlier cards' media, double-send of drafts, main-thread disk I/O and missing process timeouts.
  The orchestrator then confirmed the fix in a RELEASE build (where the real CSP applies): the guarded command
  refused `run` while playback still worked. Also: scripted edits mangled escapes a 4th time (a throwaway
  probe test) — the compiler caught it each time, so the rule stays a rule, not a guard.
- 2026-11-08: Without the target content (no real episode available), a GENERATED stress input of realistic size
  and shape found what three code reviews and 60+ UI tests missed: a 24-minute fansub-style file (overlapping speakers,
  karaoke OP, signs) showed only one of two simultaneous lines in 2 of 25 random seeks on the released build. Build the
  realistic stress input early (size, overlaps, edge styling), drive the real app over it, and keep the driver.
- 2026-11-08: Brand assets as code. Keep an app icon as an HTML/SVG source plus a render script, not a one-off PNG,
  and check it at the sizes the OS really shows (16–256 px, dark and light). A concept that looks fine at 256 px
  (a play triangle) can read as generic; a cut-out glyph still reads at 16 px if the outer shape carries it. To change
  a bundler's generated installer, read its generated script after one build (Tauri:
  `target/release/nsis/x64/installer.nsi`) and hook its own functions rather than replacing the template.
- 2026-11-09: Going public. Scan the whole history first: secrets, media, and the author emails (`git log --all
  --format='%an <%ae>'`). GitHub keeps every PR's original commits in read-only `refs/pull/*`, so rewriting history in
  place does NOT remove an email from a repo that has PRs. Only a fresh repo with the rewritten history does. Keep the
  old one renamed and private, compare tree hashes old vs new for every tag, and set the checkout's `user.email`
  to the no-reply address so the leak doesn't come back.
