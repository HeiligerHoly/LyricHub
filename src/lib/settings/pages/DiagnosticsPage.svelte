<script lang="ts">
  // Settings → Diagnose (PLAN.md 9.7 #8, early part): player session, playback clock, lyrics,
  // track-change latency (DoD Phase 1: ≤ 300 ms), overlay window state and the overlay
  // self-test from Spike B. The Sync Lab follows in Phase 3.
  import { onMount } from "svelte";
  import { formatMs, ipc } from "$lib/shared/ipc";
  import type {
    ClockEvent,
    LatencySample,
    LyricsEvent,
    NowPlaying,
    OverlayDiagnostics,
    SelfTestReport,
  } from "$lib/shared/types";

  let { now }: { now: NowPlaying } = $props();

  let diag = $state<OverlayDiagnostics | null>(null);
  let latencies = $state<LatencySample[]>([]);
  let report = $state<SelfTestReport | null>(null);
  let running = $state(false);
  let clock = $state<ClockEvent | null>(null);
  let lyrics = $state<LyricsEvent | null>(null);

  async function refresh() {
    try {
      diag = await ipc.overlayDiagnostics();
      latencies = (await ipc.trackLatencies()).slice().reverse();
      clock = await ipc.getClock();
      lyrics = await ipc.getLyrics();
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

<h1>Diagnose</h1>

<section class="card">
  <h2>Spotify (Windows-Media-Session)</h2>
  <dl class="kv">
    <dt>Status</dt>
    <dd>{now.status}</dd>
    {#if now.track}
      <dt>Titel</dt>
      <dd>{now.track.title} – {now.track.artist}{now.track.album ? ` · ${now.track.album}` : ""}</dd>
      <dt>Dauer</dt>
      <dd>{now.track.duration_ms ? formatMs(now.track.duration_ms) : "–"}</dd>
      <dt>Schlüssel</dt>
      <dd class="mono">{now.track.key}</dd>
      <dt>Cover</dt>
      <dd>{now.track.art ? `${Math.round((now.track.art.url.length * 3) / 4 / 1024)} KB` : "keins"}</dd>
    {/if}
  </dl>
</section>

<section class="card">
  <h2>Clock und Lyrics</h2>
  <dl class="kv">
    <dt>Position (Modell)</dt>
    <dd>
      {clock ? `${formatMs(clock.pos_ms)} · ${clock.playing ? "läuft" : "steht"} · Tempo ${clock.rate.toFixed(3)}` : "–"}
      {clock && clock.slew_ms > 0 ? ` · gleicht noch ${Math.round(clock.slew_ms)} ms an` : ""}
    </dd>
    <dt>Offset</dt>
    <dd>{clock ? `${clock.offset_ms} ms` : "–"}</dd>
    <dt>Anker-Nummer</dt>
    <dd>{clock?.seq ?? "–"}</dd>
    <dt>Lyrics</dt>
    <dd>
      {#if lyrics}
        {lyrics.state} · {lyrics.source || "–"} · {lyrics.lines.filter((l) => l.text).length} Zeilen{lyrics.lrc_offset_ms
          ? ` · LRC-Offset ${lyrics.lrc_offset_ms} ms`
          : ""}
      {:else}
        –
      {/if}
    </dd>
  </dl>
  <p class="hint">Aktualisiert alle 2 s. Das Overlay rechnet die Position selbst weiter und braucht diese Abfrage nicht.</p>
</section>

<section class="card">
  <h2>Titelwechsel bis zur Anzeige</h2>
  <p class="hint">
    Vom ersten Anzeichen in der Media-Session (neue Timeline oder neuer Titel) bis das Overlay den neuen Titel
    gezeichnet hat. Ziel: höchstens 300 ms.
  </p>
  {#if latencies.length}
    <p class="summary">
      {latencies.length} Wechsel · schlechtester Wert
      <span class="badge" class:pass={worst <= 300} class:fail={worst > 300}>{Math.round(worst)} ms</span>
    </p>
    <table>
      <thead><tr><th>Zeit</th><th>Titel</th><th>Gesamt</th><th>Event → Bild</th></tr></thead>
      <tbody>
        {#each latencies as l (l.unix_ms)}
          <tr>
            <td>{new Date(l.unix_ms).toLocaleTimeString("de-DE")}</td>
            <td>{l.title} – {l.artist}</td>
            <td>{Math.round(l.first_sign_to_render_ms)} ms</td>
            <td>{Math.round(l.emit_to_render_ms)} ms</td>
          </tr>
        {/each}
      </tbody>
    </table>
  {:else}
    <p class="muted">Noch kein Titelwechsel seit dem Start.</p>
  {/if}
</section>

<section class="card">
  <h2>Overlay-Fenster</h2>
  {#if diag}
    <dl class="kv">
      <dt>Sichtbar</dt>
      <dd>{diag.visible ? "ja" : "nein"}</dd>
      <dt>Modus</dt>
      <dd>{diag.edit_mode ? "Edit-Modus (fängt Klicks)" : diag.click_through ? "klickdurchlässig" : "fängt Klicks"}</dd>
      <dt>Fenster</dt>
      <dd>{diag.bounds}</dd>
      <dt>Fensterstil</dt>
      <dd class="mono">{diag.ex_style_text}</dd>
      <dt>Vordergrundfenster</dt>
      <dd>{diag.foreground_title || "–"}{diag.foreground_is_overlay ? " (Overlay! Fehler)" : ""}</dd>
    </dl>
  {/if}
  <div class="actions">
    <button class="primary" onclick={runSelfTest} disabled={running}>
      {running ? "Selbsttest läuft …" : "Selbsttest ausführen"}
    </button>
  </div>
  <p class="hint">
    Der Selbsttest blendet das Overlay mehrmals ein und aus und braucht etwa 15 Sekunden. Für einen aussagekräftigen
    Fokustest vorher ein anderes Fenster in den Vordergrund holen und den Test über das Tray-Menü starten.
  </p>
  {#if report}
    <p class="summary">
      Produktionsweg: {report.passed} bestanden · {report.failed} fehlgeschlagen · Tauri-Vergleich: {report.tauri_failed}
      erwartete Befunde · Vordergrund beim Start: „{report.foreground_at_start || "–"}“{report.foreground_conclusive
        ? ""
        : " (Fokustest nicht aussagekräftig)"}
    </p>
    <table>
      <thead><tr><th>Gruppe</th><th>Prüfung</th><th>Ergebnis</th><th>Detail</th></tr></thead>
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
