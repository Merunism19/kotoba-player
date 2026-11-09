#!/usr/bin/env bash
# Launches the debug app on a video and checks it end to end (subs, screenshot, fullscreen, clean exit); Windows dev tool.
#
# usage: bash scripts/dev/probe.sh <video-path-with-forward-slashes> <tag>
# KOTOBA_EXE=target/release/kotoba-player.exe to probe a release build (embedded UI, real CSP; no Vite needed).
# Needs: `pnpm dev` running (Vite on :1420) and `cargo build -p kotoba-player`.
# Output: artifacts/probe/<tag>-*.png|webp|log. It prints the path the app received first, because a
# bad argument (e.g. a shell-quoting slip) otherwise looks exactly like an app bug.
set -u
REPO="$(cd "$(dirname "$0")/../.." && pwd)"
OUT="$REPO/artifacts/probe"; mkdir -p "$OUT"
F="$1"; TAG="$2"
export CDP_PORT=$((9300 + RANDOM % 500))
CDP="node $REPO/scripts/dev/cdp.mjs"
cd "$REPO"

# A WebView2 browser process left from a previous run is reused by the next launch (the new
# debug port is ignored and the old page answers), so wait until none for this app remain.
for i in $(seq 1 30); do
  n=$(powershell -NoProfile -Command "(Get-CimInstance Win32_Process -Filter \"Name='msedgewebview2.exe'\" | Where-Object { \$_.CommandLine -like '*com.merunism.kotoba*' } | Measure-Object).Count")
  [ "${n//[$'\r\n ']/}" = "0" ] && break; sleep 1
done

( WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS="--remote-debugging-port=$CDP_PORT" "${KOTOBA_EXE:-./target/debug/kotoba-player.exe}" "$F" > "$OUT/$TAG-app.log" 2>&1
  echo "APP_EXIT=$?" >> "$OUT/$TAG-app.log" ) &
for i in $(seq 1 20); do curl -s "http://127.0.0.1:$CDP_PORT/json" >/dev/null && break; sleep 1; done
for i in $(seq 1 30); do [ "$($CDP "!!document.querySelector('.p-top')" 2>/dev/null)" = "true" ] && break; sleep 0.5; done

I="const I = window.__TAURI_INTERNALS__.invoke; const gp = (n) => I('plugin:libmpv|get_property', {name:n, format:'string', windowLabel:'main'}).catch(e=>'ERR '+e);"
$CDP "(async () => { $I return { args: await I('startup_files'), path: await gp('path') }; })()"
$CDP "(async () => { $I await I('mpv_command', {name:'seek', args:[5,'absolute']}); await new Promise(r => setTimeout(r, 1500)); return { title: document.querySelector('.p-title')?.innerText, jp: document.querySelector('.p-jp')?.innerText, words: [...document.querySelectorAll('.p-jp .unit.word')].map(e => e.innerText).join(' | '), en: document.querySelector('.p-en')?.innerText, guard: await I('mpv_command', {name:'run', args:['cmd','/c','exit']}).then(()=>'ALLOWED (BAD)').catch(e=>'refused: '+e), rawPlugin: await I('plugin:libmpv|command', {name:'stop', args:[], windowLabel:'main'}).then(()=>'ALLOWED (BAD)').catch(e=>'refused'), toasts: [...document.querySelectorAll('.toast-msg')].map(e => e.innerText), sid: await gp('sid'), sids: await gp('track-list/count'), fps: await gp('estimated-vf-fps'), drops: await gp('frame-drop-count') }; })()"
powershell -NoProfile -ExecutionPolicy Bypass -File "$REPO/scripts/dev/window-shot.ps1" -Out "$(cygpath -w "$OUT/$TAG-window.png")"
$CDP "window.__TAURI_INTERNALS__.invoke('plugin:window|set_fullscreen', {label:'main', value:true}).then(()=>'fullscreen on').catch(e=>'FS ERR '+e)"
sleep 1.5
powershell -NoProfile -ExecutionPolicy Bypass -File "$REPO/scripts/dev/window-shot.ps1" -Out "$(cygpath -w "$OUT/$TAG-fullscreen.png")"
$CDP "window.__TAURI_INTERNALS__.invoke('plugin:window|set_fullscreen', {label:'main', value:false}).then(()=>'fullscreen off')"
sleep 1
taskkill //IM kotoba-player.exe > /dev/null 2>&1
for i in $(seq 1 10); do grep -q APP_EXIT "$OUT/$TAG-app.log" && break; sleep 1; done
grep APP_EXIT "$OUT/$TAG-app.log"
