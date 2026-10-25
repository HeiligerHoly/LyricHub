// Phase 4 DoD "no layout per frame" (PLAN.md 10): counts layouts and style recalculations of the
// overlay page over a time window through the DevTools protocol of WebView2. A fill that ran
// through layout would show one layout per frame (30+ per second); the word-by-word fill should
// cause a few per line change and nothing in between.
//
// LyricHub must run with remote debugging, e.g. in Git Bash:
//   WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9223 src-tauri/target/debug/lyrichub.exe --minimized
// then, while a song with synced lyrics plays:
//   node tools/overlay-spike/layout-count.mjs 30
const seconds = Number(process.argv[2] ?? 30);
const port = Number(process.env.CDP_PORT ?? 9223);

const targets = await (await fetch(`http://127.0.0.1:${port}/json`)).json();
const target = targets.find((t) => t.type === "page" && t.url.includes("/overlay"));
if (!target) {
  console.error("Overlay page not found. Pages:", targets.map((t) => t.url).join(", ") || "none");
  process.exit(1);
}

const ws = new WebSocket(target.webSocketDebuggerUrl);
let nextId = 0;
const waiting = new Map();
ws.onmessage = (m) => {
  const msg = JSON.parse(m.data);
  waiting.get(msg.id)?.(msg.result);
  waiting.delete(msg.id);
};
const send = (method, params = {}) =>
  new Promise((resolve) => {
    const id = ++nextId;
    waiting.set(id, resolve);
    ws.send(JSON.stringify({ id, method, params }));
  });
await new Promise((resolve) => (ws.onopen = resolve));

await send("Performance.enable");
const metrics = async () => Object.fromEntries((await send("Performance.getMetrics")).metrics.map((m) => [m.name, m.value]));
// The current line, read from the page, tells how many line changes the window had.
const line = async () => (await send("Runtime.evaluate", { expression: "document.querySelector('.line .base')?.textContent?.length ?? -1", returnByValue: true })).result.value;

const before = await metrics();
let changes = 0;
let last = await line();
const started = Date.now();
while (Date.now() - started < seconds * 1000) {
  await new Promise((r) => setTimeout(r, 250));
  const now = await line();
  if (now !== last) changes++;
  last = now;
}
const after = await metrics();
ws.close();

const d = (k) => after[k] - before[k];
const result = {
  seconds,
  line_changes: changes,
  layouts: d("LayoutCount"),
  style_recalcs: d("RecalcStyleCount"),
  layout_ms: Math.round(d("LayoutDuration") * 1000),
  style_ms: Math.round(d("RecalcStyleDuration") * 1000),
  script_ms: Math.round(d("ScriptDuration") * 1000),
  layouts_per_second: Math.round((d("LayoutCount") / seconds) * 10) / 10,
};
console.log(JSON.stringify(result, null, 2));
// One layout per frame at 30 fps would be 30 per second; allow a few per line change.
const ok = result.layouts_per_second < 5;
console.log(ok ? "PASS: no layout per frame" : "FAIL: layouts run per frame");
process.exit(ok ? 0 : 1);
