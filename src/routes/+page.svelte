<script lang="ts">
  // Settings window, Phase 0: Spotify connection check (Rust WinRT path) and Spike B tools.
  // The full settings pages from PLAN.md 9.7 follow in Phase 1 and later.
  import { onMount } from "svelte";
  import {
    formatMs,
    ipc,
    type NowPlaying,
    type OverlayDiagnostics,
    type SelfTestReport,
  } from "$lib/ipc";

  let nowPlaying = $state<NowPlaying | null>(null);
  let smtcError = $state<string | null>(null);
  let diag = $state<OverlayDiagnostics | null>(null);
  let report = $state<SelfTestReport | null>(null);
  let selfTestRunning = $state(false);

  async function refreshNowPlaying() {
    try {
      nowPlaying = await ipc.nowPlaying();
      smtcError = null;
    } catch (e) {
      smtcError = String(e);
    }
  }

  async function refreshDiag() {
    try {
      diag = await ipc.overlayDiagnostics();
    } catch {
      diag = null;
    }
  }

  async function toggleOverlay() {
    await ipc.toggleOverlay();
    await refreshDiag();
  }

  async function toggleClickThrough() {
    diag = await ipc.setClickThrough(!(diag?.click_through ?? true));
  }

  async function runSelfTest() {
    selfTestRunning = true;
    try {
      report = await ipc.runOverlaySelfTest();
    } finally {
      selfTestRunning = false;
      await refreshDiag();
    }
  }

  onMount(() => {
    refreshNowPlaying();
    refreshDiag();
    const id = setInterval(() => {
      if (document.visibilityState === "visible") {
        refreshNowPlaying();
        refreshDiag();
      }
    }, 1000);
    return () => clearInterval(id);
  });
</script>

<main class="settings">
  <header class="top">
    <h1>LyricHub</h1>
    <span class="phase">v0.0.1 · Phase 0</span>
  </header>

  <section class="card">
    <h2>Spotify</h2>
    {#if smtcError}
      <p class="error">Windows-Media-Session nicht lesbar: {smtcError}</p>
    {:else if nowPlaying}
      <div class="np">
        <div>
          <p class="np-title">{nowPlaying.title}</p>
          <p class="np-sub">{nowPlaying.artist}{nowPlaying.album ? ` · ${nowPlaying.album}` : ""}</p>
        </div>
        <span class="badge" class:ok={nowPlaying.status === "Playing"}>{nowPlaying.status}</span>
      </div>
      <dl class="kv">
        <dt>Position</dt>
        <dd>{formatMs(nowPlaying.position_now_ms)} / {formatMs(nowPlaying.end_ms)}</dd>
        <dt>Letztes Update von Spotify</dt>
        <dd>vor {(nowPlaying.last_update_age_ms / 1000).toFixed(1)} s</dd>
        <dt>Session</dt>
        <dd class="mono">{nowPlaying.app_id}</dd>
      </dl>
    {:else}
      <p class="muted">Keine Spotify-Session gefunden. Läuft Spotify?</p>
    {/if}
  </section>

  <section class="card">
    <h2>Overlay-Test (Spike B)</h2>
    <div class="actions">
      <button onclick={toggleOverlay}>Overlay ein/aus</button>
      <button onclick={toggleClickThrough}>Klickdurchlässig {diag?.click_through ? "aus" : "an"}</button>
      <button class="primary" onclick={runSelfTest} disabled={selfTestRunning}>
        {selfTestRunning ? "Selbsttest läuft …" : "Selbsttest ausführen"}
      </button>
    </div>
    {#if diag}
      <dl class="kv">
        <dt>Sichtbar</dt>
        <dd>{diag.visible ? "ja" : "nein"}</dd>
        <dt>Klickdurchlässig</dt>
        <dd>{diag.click_through ? "ja" : "nein (Edit-Modus)"}</dd>
        <dt>Fensterstil</dt>
        <dd class="mono">{diag.ex_style_text}</dd>
        <dt>Vordergrundfenster</dt>
        <dd>{diag.foreground_title || "–"}{diag.foreground_is_overlay ? " (Overlay! Fehler)" : ""}</dd>
      </dl>
    {/if}
    <p class="hint">
      Der Selbsttest blendet das Overlay mehrmals ein und aus und braucht etwa 15 Sekunden. Für einen
      aussagekräftigen Fokustest vorher ein anderes Fenster in den Vordergrund holen und den Test über das
      Tray-Menü starten.
    </p>
    {#if report}
      <p class="summary">
        Produktionsweg: {report.passed} bestanden · {report.failed} fehlgeschlagen · Tauri-Vergleich:
        {report.tauri_failed} erwartete Befunde · Vordergrund beim Start: „{report.foreground_at_start || "–"}“{report.foreground_conclusive
          ? ""
          : " (Fokustest nicht aussagekräftig)"}
      </p>
      <table class="steps">
        <thead><tr><th>Gruppe</th><th>Prüfung</th><th>Ergebnis</th><th>Detail</th></tr></thead>
        <tbody>
          {#each report.steps as step}
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

  <section class="card">
    <h2>Tastenkürzel</h2>
    <dl class="kv">
      <dt>Strg+Alt+L</dt>
      <dd>Overlay ein/aus</dd>
      <dt>Strg+Alt+E</dt>
      <dd>Klickdurchlässigkeit umschalten (Vorstufe des Edit-Modus)</dd>
    </dl>
    <p class="hint">Logs: <span class="mono">%APPDATA%\LyricHub\logs</span></p>
  </section>
</main>

<style>
  :global(:root) {
    --bg: #111318;
    --card: #1a1d24;
    --line: #2a2f3a;
    --text: #e8eaf0;
    --muted: #9aa1b2;
    --accent: #ffd36e;
    --ok: #4ade80;
    --fail: #f87171;
    color-scheme: dark;
  }
  :global(body) {
    margin: 0;
    background: var(--bg);
    color: var(--text);
    font-family: "Inter Variable", "Segoe UI Variable", "Segoe UI", sans-serif;
    font-size: 14px;
  }

  .settings {
    max-width: 880px;
    margin: 0 auto;
    padding: 24px;
    display: grid;
    gap: 16px;
  }
  .top {
    display: flex;
    align-items: baseline;
    gap: 12px;
  }
  h1 {
    margin: 0;
    font-size: 22px;
  }
  h2 {
    margin: 0 0 12px;
    font-size: 15px;
  }
  .phase,
  .muted,
  .hint {
    color: var(--muted);
  }
  .hint {
    font-size: 12px;
    margin: 12px 0 0;
  }
  .card {
    background: var(--card);
    border: 1px solid var(--line);
    border-radius: 12px;
    padding: 16px 18px;
  }
  .np {
    display: flex;
    justify-content: space-between;
    align-items: flex-start;
    gap: 12px;
  }
  .np-title {
    margin: 0;
    font-size: 16px;
    font-weight: 600;
  }
  .np-sub {
    margin: 2px 0 0;
    color: var(--muted);
  }
  .kv {
    display: grid;
    grid-template-columns: 200px 1fr;
    gap: 6px 12px;
    margin: 12px 0 0;
  }
  .kv dt {
    color: var(--muted);
  }
  .kv dd {
    margin: 0;
  }
  .mono {
    font-family: "Cascadia Mono", Consolas, monospace;
    font-size: 12px;
  }
  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }
  button {
    background: #262b36;
    color: var(--text);
    border: 1px solid var(--line);
    border-radius: 8px;
    padding: 7px 12px;
    font: inherit;
    cursor: pointer;
  }
  button:hover:not(:disabled) {
    border-color: var(--muted);
  }
  button.primary {
    background: var(--accent);
    color: #1a1400;
    border-color: transparent;
  }
  button:disabled {
    opacity: 0.6;
    cursor: default;
  }
  .badge {
    display: inline-block;
    padding: 1px 8px;
    border-radius: 999px;
    font-size: 12px;
    background: #2a2f3a;
    color: var(--muted);
  }
  .badge.ok,
  .badge.pass {
    background: rgba(74, 222, 128, 0.15);
    color: var(--ok);
  }
  .badge.fail {
    background: rgba(248, 113, 113, 0.15);
    color: var(--fail);
  }
  .error {
    color: var(--fail);
  }
  .summary {
    margin: 14px 0 8px;
  }
  .steps {
    width: 100%;
    border-collapse: collapse;
    font-size: 12px;
  }
  .steps th,
  .steps td {
    text-align: left;
    padding: 5px 6px;
    border-top: 1px solid var(--line);
    vertical-align: top;
  }
  .steps th {
    color: var(--muted);
    font-weight: 500;
  }
  .detail {
    color: var(--muted);
    word-break: break-word;
  }
</style>
