<script lang="ts">
  // Settings → Diagnose (PLAN.md 9.7 #8): player session, Sync Lab (5.9), lyrics, track-change
  // latency, overlay window state and the overlay self-test from Spike B.
  import { onMount } from "svelte";
  import SyncLab from "../SyncLab.svelte";
  import { lang, t, type Key } from "#lib/shared/i18n.svelte.js";
  import { formatMs, ipc } from "#lib/shared/ipc.js";
  import type {
    LatencySample,
    LyricsEvent,
    NowPlaying,
    OverlayDiagnostics,
    PluginInfo,
    SelfTestReport,
  } from "#lib/shared/types.js";

  let { now }: { now: NowPlaying } = $props();

  let diag = $state<OverlayDiagnostics | null>(null);
  let latencies = $state<LatencySample[]>([]);
  let report = $state<SelfTestReport | null>(null);
  let running = $state(false);
  let lyrics = $state<LyricsEvent | null>(null);
  let plugins = $state<PluginInfo[]>([]);

  async function refresh() {
    try {
      diag = await ipc.overlayDiagnostics();
      latencies = (await ipc.trackLatencies()).slice().reverse();
      lyrics = await ipc.getLyrics();
      plugins = await ipc.listPlugins();
    } catch {
      diag = null;
    }
  }

  async function runSelfTest() {
    running = true;
    try {
      report = await ipc.runOverlaySelfTest();
    } finally {
      running = false;
      refresh();
    }
  }

  onMount(() => {
    refresh();
    // One IPC call every 2 s while this page is open; never per frame.
    const id = setInterval(() => document.visibilityState === "visible" && refresh(), 2000);
    return () => clearInterval(id);
  });

  const worst = $derived(latencies.reduce((m, l) => Math.max(m, l.first_sign_to_render_ms), 0));
</script>

<h1>{t("nav.diagnostics")}</h1>

<section class="card">
  <h2>{t("diag.spotify")}</h2>
  <dl class="kv">
    <dt>{t("diag.status")}</dt>
    <dd>{now.status}</dd>
    {#if now.track}
      <dt>{t("diag.track")}</dt>
      <dd>{now.track.title} – {now.track.artist}{now.track.album ? ` · ${now.track.album}` : ""}</dd>
      <dt>{t("diag.duration")}</dt>
      <dd>{now.track.duration_ms ? formatMs(now.track.duration_ms) : "–"}</dd>
      <dt>{t("diag.key")}</dt>
      <dd class="mono">{now.track.key}</dd>
      <dt>{t("diag.cover")}</dt>
      <dd>{now.track.art ? `${Math.round((now.track.art.url.length * 3) / 4 / 1024)} KB` : t("diag.cover.none")}</dd>
    {/if}
  </dl>
</section>

<section class="card">
  <h2>{t("diag.plugins")}</h2>
  {#if plugins.length}
    <dl class="kv">
      {#each plugins as p (p.id)}
        <dt>{p.name}</dt>
        <dd>{p.version} · {t(`plugins.state.${p.state}` as Key)}{p.detail && p.state !== "restarting" ? ` · ${p.detail}` : ""}</dd>
      {/each}
    </dl>
  {:else}
    <p class="muted">{t("diag.plugins.none")}</p>
  {/if}
</section>

<SyncLab />

<section class="card">
  <h2>{t("diag.lyrics")}</h2>
  <dl class="kv">
    <dt>{t("diag.lyrics")}</dt>
    <dd>
      {#if lyrics}
        {lyrics.state} · {lyrics.source || "–"} · {t("diag.lyrics.lines", { n: lyrics.lines.filter((l) => l.text).length })}{lyrics.lrc_offset_ms
          ? ` · ${t("diag.lyrics.lrcOffset", { ms: lyrics.lrc_offset_ms })}`
          : ""}
      {:else}
        –
      {/if}
    </dd>
    {#if lyrics?.word_source}
      <dt>{t("diag.words")}</dt>
      <dd>
        {lyrics.word_source.startsWith("plugin:")
          ? t("diag.words.plugin", { id: lyrics.word_source.slice("plugin:".length) })
          : t(lyrics.word_source === "lrc" ? "diag.words.lrc" : "diag.words.estimate")}
      </dd>
    {/if}
    {#if lyrics?.line_sync_ms != null}
      <dt>{t("diag.lineSync")}</dt>
      <dd>{t("diag.lineSync.value", { ms: lyrics.line_sync_ms })}</dd>
    {/if}
    {#if lyrics && /plugin:[^:]+:/.test(lyrics.source)}
      <dt>{t("diag.lyricsOrigin")}</dt>
      <dd>{t(`diag.lyricsOrigin.${lyrics.source.split(":").pop()}` as Key)}</dd>
    {/if}
  </dl>
</section>

<section class="card">
  <h2>{t("diag.latency.title")}</h2>
  <p class="hint">{t("diag.latency.hint")}</p>
  {#if latencies.length}
    <p class="summary">
      {t("diag.latency.summary", { n: latencies.length })}
      <span class="badge" class:pass={worst <= 300} class:fail={worst > 300}>{Math.round(worst)} ms</span>
    </p>
    <table>
      <thead>
        <tr><th>{t("diag.latency.time")}</th><th>{t("diag.track")}</th><th>{t("diag.latency.total")}</th><th>{t("diag.latency.frame")}</th></tr>
      </thead>
      <tbody>
        {#each latencies as l (l.unix_ms)}
          <tr>
            <td>{new Date(l.unix_ms).toLocaleTimeString(lang() === "de" ? "de-DE" : "en-US")}</td>
            <td>{l.title} – {l.artist}</td>
            <td>{Math.round(l.first_sign_to_render_ms)} ms</td>
            <td>{Math.round(l.emit_to_render_ms)} ms</td>
          </tr>
        {/each}
      </tbody>
    </table>
  {:else}
    <p class="muted">{t("diag.latency.none")}</p>
  {/if}
</section>

<section class="card">
  <h2>{t("diag.window.title")}</h2>
  {#if diag}
    <dl class="kv">
      <dt>{t("diag.window.visible")}</dt>
      <dd>{t(diag.visible ? "diag.yes" : "diag.no")}</dd>
      <dt>{t("diag.window.mode")}</dt>
      <dd>{t(diag.edit_mode ? "diag.window.mode.edit" : diag.click_through ? "diag.window.mode.clickThrough" : "diag.window.mode.catches")}</dd>
      <dt>{t("diag.window.bounds")}</dt>
      <dd>{diag.bounds}</dd>
      <dt>{t("diag.window.style")}</dt>
      <dd class="mono">{diag.ex_style_text}</dd>
      <dt>{t("diag.window.foreground")}</dt>
      <dd>{diag.foreground_app || "–"}{diag.foreground_is_overlay ? ` ${t("diag.window.foregroundIsOverlay")}` : ""}</dd>
      <dt>{t("diag.window.screen")}</dt>
      <dd>{t(`diag.window.screen.${diag.guard.screen}`)}{diag.auto_hidden ? ` · ${t("diag.window.autoHidden")}` : ""}</dd>
      <dt>{t("diag.window.guard")}</dt>
      <dd>
        {t("diag.window.guard.value", { n: diag.guard.reasserts })}{diag.guard.covered_by
          ? ` · ${t("diag.window.guard.covered", { window: diag.guard.covered_by })}`
          : ""}
      </dd>
    </dl>
  {/if}
  <div class="actions">
    <button class="primary" onclick={runSelfTest} disabled={running}>
      {running ? t("diag.selftest.running") : t("diag.selftest.run")}
    </button>
  </div>
  <p class="hint">{t("diag.selftest.hint")}</p>
  {#if report}
    <p class="summary">
      {t("diag.selftest.summary", {
        passed: report.passed,
        failed: report.failed,
        tauri: report.tauri_failed,
        foreground: report.foreground_at_start || "–",
      })}{report.foreground_conclusive ? "" : ` ${t("diag.selftest.inconclusive")}`}
    </p>
    <!-- Group, check and detail come from the backend self-test and stay German. -->
    <table>
      <thead>
        <tr><th>{t("diag.selftest.group")}</th><th>{t("diag.selftest.check")}</th><th>{t("diag.selftest.result")}</th><th>{t("diag.selftest.detail")}</th></tr>
      </thead>
      <tbody>
        {#each report.steps as step, i (i)}
          <tr>
            <td>{step.group}</td>
            <td>{step.name}</td>
            <td><span class="badge {step.status}">{step.status}</span></td>
            <td class="detail">{step.detail}</td>
          </tr>
        {/each}
      </tbody>
    </table>
  {/if}
</section>

<style>
  h1 {
    margin: 0 0 18px;
    font-size: 22px;
  }
  h2 {
    margin: 0 0 8px;
    font-size: 15px;
  }
  .card {
    background: var(--card);
    border: 1px solid var(--line);
    border-radius: 12px;
    padding: 16px 18px;
    margin-bottom: 14px;
  }
  .hint {
    margin: 0 0 8px;
    color: var(--muted);
    font-size: 12.5px;
  }
  .kv {
    display: grid;
    grid-template-columns: 170px 1fr;
    gap: 6px 12px;
    margin: 0;
  }
  .kv dt {
    color: var(--muted);
  }
  .kv dd {
    margin: 0;
    word-break: break-word;
  }
  .actions {
    margin: 12px 0 8px;
  }
  .summary {
    margin: 10px 0 8px;
  }
  table {
    width: 100%;
    border-collapse: collapse;
    font-size: 12px;
  }
  th,
  td {
    text-align: left;
    padding: 5px 6px;
    border-top: 1px solid var(--line);
    vertical-align: top;
  }
  th {
    color: var(--muted);
    font-weight: 500;
  }
  .detail {
    color: var(--muted);
    word-break: break-word;
  }
  .badge {
    display: inline-block;
    padding: 1px 8px;
    border-radius: 999px;
    font-size: 12px;
    background: #2a2f3a;
    color: var(--muted);
  }
  .badge.pass {
    background: rgba(74, 222, 128, 0.15);
    color: var(--ok);
  }
  .badge.fail {
    background: rgba(248, 113, 113, 0.15);
    color: var(--fail);
  }
</style>
