// Drives the running app (over CDP_PORT) through an episode-length session and prints timings: open, random seeks (shown lines vs cues), lookups, 60 s of playback (events/s, heap, DOM). Dev tool.
//
// usage: CDP_PORT=9333 node scripts/dev/stress.mjs "<video path>"   (start the app with probe.sh's env first)
const video = process.argv[2];
const port = process.env.CDP_PORT || 9222;

async function connect() {
  for (let i = 0; i < 60; i++) {
    try {
      const list = await (await fetch(`http://127.0.0.1:${port}/json`)).json();
      const page = list.find((t) => t.type === 'page');
      if (page) {
        const ws = new WebSocket(page.webSocketDebuggerUrl);
        await new Promise((r, j) => { ws.addEventListener('open', r); ws.addEventListener('error', j); });
        return ws;
      }
    } catch { /* not up yet */ }
    await new Promise((r) => setTimeout(r, 500));
  }
  throw new Error('no CDP page');
}

const ws = await connect();
let nextId = 1;
function evaluate(expression) {
  const id = nextId++;
  ws.send(JSON.stringify({ id, method: 'Runtime.evaluate', params: { expression, awaitPromise: true, returnByValue: true } }));
  return new Promise((resolve) => {
    const on = (e) => {
      const m = JSON.parse(e.data);
      if (m.id !== id) return;
      ws.removeEventListener('message', on);
      resolve(m.result?.result?.value ?? { error: m.result?.exceptionDetails?.exception?.description ?? m });
    };
    ws.addEventListener('message', on);
  });
}

const v = JSON.stringify(video);
// Wait for the real page (a dev build first shows a loading page without the app's IPC).
const tWait = Date.now();
while (Date.now() - tWait < 120_000) {
  const ready = await evaluate("!!(window.__TAURI_INTERNALS__ && document.querySelector('.p-top'))");
  if (ready === true) break;
  await new Promise((r) => setTimeout(r, 500));
}
const result = await evaluate(`(async () => {
  const I = window.__TAURI_INTERNALS__.invoke;
  const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
  const out = {};
  const gp = (n) => I('plugin:libmpv|get_property', {name: n, format: 'string', windowLabel: 'main'}).catch(() => null);
  // 1) open (the app was started with the file on the command line)
  const t0 = performance.now();
  for (let i = 0; i < 240 && !document.querySelector('.p-top'); i++) await sleep(250);
  out.openMs = Math.round(performance.now() - t0) + ' (after page load)';
  const p = await I('prepare_media', {path: ${v}});
  const jp = await I('get_cues', {path: ${v}, subId: p.jpSubId});
  const dialogue = jp.filter((c) => c.kind === 'dialogue');
  out.cues = jp.length + ' (' + dialogue.length + ' dialogue)';
  // 2) random seeks: the shown lines must be exactly the dialogue cues active at that time
  let seed = 7; const rnd = () => ((seed = (seed * 1103515245 + 12345) % 2147483648) / 2147483648);
  const dur = Number(await gp('duration'));
  let mismatches = []; let wordsMissing = 0;
  for (let k = 0; k < 25; k++) {
    const at = 5 + rnd() * (dur - 20);
    await I('mpv_command', {name: 'seek', args: [at, 'absolute+exact']});
    await I('mpv_set', {name: 'pause', value: true});
    await sleep(700);
    const t = Number(await gp('time-pos')) * 1000;
    const want = dialogue.filter((c) => c.startMs <= t && t < c.endMs).map((c) => c.text.replace(/\\n/g, ''));
    const shown = [...document.querySelectorAll('.p-jp')].map((e) => e.innerText.replace(/\\s/g, ''));
    const same = want.length === shown.length && want.every((w, i) => shown[i] && shown[i].startsWith(w.replace(/\\s/g, '').slice(0, 6)));
    if (!same) mismatches.push({ t: Math.round(t), want, shown });
    if (want.length && !document.querySelector('.p-jp .unit.word')) wordsMissing++;
  }
  out.seeks = 25; out.lineMismatches = mismatches.length; out.mismatchExamples = mismatches.slice(0, 3); out.seeksWithoutWordUnits = wordsMissing;
  // 3) lookups on real lines
  const times = [];
  for (const c of dialogue.slice(0, 40).filter((_, i) => i % 4 === 0)) {
    const s = performance.now();
    await I('lookup_at', {text: c.text, offset: 0});
    times.push(performance.now() - s);
  }
  times.sort((a, b) => a - b);
  out.lookupMs = { p50: +times[Math.floor(times.length / 2)].toFixed(1), max: +times[times.length - 1].toFixed(1) };
  // 4) play 60 s from the OP (karaoke + signs) and watch the page
  await I('mpv_command', {name: 'seek', args: [85, 'absolute']});
  await I('mpv_set', {name: 'pause', value: false});
  const heap0 = performance.memory ? performance.memory.usedJSHeapSize : 0;
  const nodes0 = document.getElementsByTagName('*').length;
  let renders = 0; const obs = new MutationObserver((m) => { renders += m.length; });
  obs.observe(document.body, { subtree: true, childList: true, characterData: true, attributes: true });
  const pos0 = Number(await gp('time-pos'));
  await sleep(60000);
  obs.disconnect();
  const pos1 = Number(await gp('time-pos'));
  out.play = {
    seconds: +(pos1 - pos0).toFixed(1),
    drops: await gp('frame-drop-count'),
    hwdec: await gp('hwdec-current'),
    domMutationsPerSec: Math.round(renders / 60),
    domNodes: nodes0 + ' -> ' + document.getElementsByTagName('*').length,
    heapMB: (heap0 / 1e6).toFixed(1) + ' -> ' + ((performance.memory ? performance.memory.usedJSHeapSize : 0) / 1e6).toFixed(1),
  };
  await I('mpv_set', {name: 'pause', value: true});
  return out;
})()`);
console.log(JSON.stringify(result, null, 1));
ws.close();
