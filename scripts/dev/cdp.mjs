// Evaluates a JS expression in the running app's WebView2 over the remote-debugging port (CDP_PORT, default 9222); dev tool.
// usage: CDP_PORT=9333 node scripts/dev/cdp.mjs "<expression>"
const expr = process.argv[2];
const list = await (await fetch('http://127.0.0.1:' + (process.env.CDP_PORT || 9222) + '/json')).json();
const page = list.find((t) => t.type === 'page');
const ws = new WebSocket(page.webSocketDebuggerUrl);
await new Promise((r) => ws.addEventListener('open', r));
ws.send(JSON.stringify({ id: 1, method: 'Runtime.evaluate', params: { expression: expr, awaitPromise: true, returnByValue: true } }));
const msg = await new Promise((r) => ws.addEventListener('message', (e) => r(JSON.parse(e.data))));
console.log(JSON.stringify(msg.result?.result?.value ?? msg.result ?? msg, null, 1));
ws.close();
