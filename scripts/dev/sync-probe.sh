#!/usr/bin/env bash
# End-to-end check of "sync to voices" in the real app (Windows dev tool).
#
# usage: bash scripts/dev/sync-probe.sh [case=const]
# Needs: node scripts/make-speech-sample.mjs, and a build (KOTOBA_EXE, default the release build).
# Copies samples/speech/<case>/ (Ep.mkv + an out-of-sync Ep.ja.srt and Ep.en.srt) to a fresh folder
# (the app syncs each file once, remembered by path), opens it, waits for the automatic sync, then
# seeks into several spoken lines and checks the Japanese AND English shown are the line being spoken.
# Output: artifacts/probe/sync-<case>-*.png|log and a PASS/FAIL line per checked line.
set -u
REPO="$(cd "$(dirname "$0")/../.." && pwd)"
REPO_W="$(cd "$REPO" && pwd -W)" # Windows form, for node
CASE="${1:-const}"
OUT="$REPO/artifacts/probe"; mkdir -p "$OUT"
RUN="$REPO/samples/speech/probe-$CASE-$(date +%s)"
mkdir -p "$RUN" && cp "$REPO/samples/speech/$CASE/"Ep.* "$RUN/"
F="$(cd "$RUN" && pwd -W)/Ep.mkv"
export CDP_PORT=$((9300 + RANDOM % 500))
CDP="node $REPO/scripts/dev/cdp.mjs"
echo "video: $F"

for i in $(seq 1 30); do
  n=$(powershell -NoProfile -Command "(Get-CimInstance Win32_Process -Filter \"Name='msedgewebview2.exe'\" | Where-Object { \$_.CommandLine -like '*com.merunism.kotoba*' } | Measure-Object).Count")
  [ "${n//[$'\r\n ']/}" = "0" ] && break; sleep 1
done
( WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS="--remote-debugging-port=$CDP_PORT" "${KOTOBA_EXE:-$REPO/target/release/kotoba-player.exe}" "$F" > "$OUT/sync-$CASE-app.log" 2>&1
  echo "APP_EXIT=$?" >> "$OUT/sync-$CASE-app.log" ) &
for i in $(seq 1 20); do curl -s "http://127.0.0.1:$CDP_PORT/json" >/dev/null && break; sleep 1; done
for i in $(seq 1 30); do [ "$($CDP "!!document.querySelector('.p-top')" 2>/dev/null)" = "true" ] && break; sleep 0.5; done
sleep 5 # the automatic sync (decode + detect + align: ~0.5 s for 6 min) and the reload
$CDP "[...document.querySelectorAll('.toast-msg')].map(e => e.innerText)"

# Lines to check: true speech times from truth.json; expected English from the English file's text.
CHECKS=$(node -e "
const t = require('$REPO_W/samples/speech/truth.json').lines;
const en = require('fs').readFileSync('$REPO_W/samples/speech/$CASE/Ep.en.srt', 'utf8').split(/\r?\n\r?\n/).filter(Boolean).map(b => b.split(/\r?\n/).slice(2).join(' '));
const pick = [3, Math.floor(t.length / 3), Math.floor(t.length / 2), t.length - 4];
console.log(JSON.stringify(pick.map(i => ({ at: (t[i].start + 100 + 300) / 1000, jp: t[i].jp, en: en[i] }))));
")
I="const I = window.__TAURI_INTERNALS__.invoke;"
$CDP "(async () => { $I const checks = $CHECKS; const out = [];
  for (const c of checks) {
    await I('mpv_command', {name:'seek', args:[c.at, 'absolute']}); await new Promise(r => setTimeout(r, 900));
    const jp = [...document.querySelectorAll('.p-jp')].map(e => e.innerText.replace(/\s+/g, '')).join(' / ');
    const en = [...document.querySelectorAll('.p-en')].map(e => e.innerText.trim()).join(' / ');
    out.push((jp.includes(c.jp.replace(/\s+/g, '')) && en.includes(c.en) ? 'PASS' : 'FAIL') + ' at ' + c.at + ' s: jp=' + jp + ' | en=' + en + ' | want ' + c.jp + ' | ' + c.en);
  }
  return out; })()"
powershell -NoProfile -ExecutionPolicy Bypass -File "$REPO/scripts/dev/window-shot.ps1" -Out "$(cygpath -w "$OUT/sync-$CASE-window.png")"
taskkill //IM kotoba-player.exe > /dev/null 2>&1
for i in $(seq 1 10); do grep -q APP_EXIT "$OUT/sync-$CASE-app.log" && break; sleep 1; done
rm -rf "$RUN"
