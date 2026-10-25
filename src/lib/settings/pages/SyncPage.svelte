<script lang="ts">
  // Settings → Synchronisation (PLAN.md 9.7 #4): global offset, the offset of the current song
  // (set by the nudge hotkeys, kept per song), offsets per audio device, the calibration
  // assistant (5.7) and the tap-based sync protocol (11.3), plus the lyrics of the current song.
  import { onMount, untrack } from "svelte";
  import { ipc, on, subscribe } from "$lib/shared/ipc";
  import {
    OFFSET_LIMIT_MS,
    type AudioDevice,
    type CalibrationView,
    type LyricsEvent,
    type NowPlaying,
    type Offsets,
    type Settings,
    type SettingsPatch,
    type TapStats,
  } from "$lib/shared/types";

  interface Props {
    settings: Settings;
    update: (patch: SettingsPatch) => Promise<void>;
    now: NowPlaying;
  }
  let { settings, update, now }: Props = $props();

  const SLIDER_LIMIT = 2000;
  const PATCH_DELAY_MS = 80;

  // ---- Global offset ----
  let offset = $state(untrack(() => settings.sync.global_offset_ms));
  let lastInput = 0;
  $effect(() => {
    const next = settings.sync.global_offset_ms;
    if (performance.now() - lastInput > 400) offset = next;
  });
  let timer: ReturnType<typeof setTimeout> | undefined;
  function setOffset(value: number) {
    offset = Math.max(-OFFSET_LIMIT_MS, Math.min(OFFSET_LIMIT_MS, Math.round(value)));
    lastInput = performance.now();
    clearTimeout(timer);
    timer = setTimeout(() => update({ sync: { global_offset_ms: offset } }), PATCH_DELAY_MS);
  }

  // ---- Song and device offsets ----
  let offsets = $state<Offsets | null>(null);
  let devices = $state<AudioDevice[]>([]);
  const refreshDevices = () => ipc.listAudioDevices().then((d) => (devices = d));

  let deviceTimers: Record<string, ReturnType<typeof setTimeout>> = {};
  function setDevice(id: string, value: number) {
    clearTimeout(deviceTimers[id]);
    deviceTimers[id] = setTimeout(() => ipc.setDeviceOffset(id, Math.round(value)).then((d) => (devices = d)), 300);
  }

  // ---- Calibration and protocol ----
  let calibration = $state<CalibrationView | null>(null);
  let startError = $state<string | null>(null);
  async function startMeasure(mode: "calibrate" | "protocol") {
    startError = null;
    try {
      calibration = await ipc.startCalibration(mode);
    } catch (e) {
      startError = String(e);
    }
  }
  async function applySuggestion() {
    await ipc.applyCalibration();
    refreshDevices();
  }

  // ---- Lyrics ----
  let lyrics = $state<LyricsEvent | null>(null);
  let reloading = $state(false);
  const current = $derived(lyrics && now.track && lyrics.key === now.track.key ? lyrics : null);
  const stateText: Record<LyricsEvent["state"], string> = {
    loading: "wird gesucht …",
    synced: "synchron",
    unsynced: "nur ohne Zeitangaben",
    instrumental: "Instrumental",
    missing: "nicht gefunden",
    error: "LRCLIB nicht erreichbar, neuer Versuch läuft",
  };
  async function reload() {
    reloading = true;
    await ipc.reloadLyrics();
    setTimeout(() => (reloading = false), 1500);
  }

  const describe = (ms: number) =>
    ms === 0 ? "keine Verschiebung" : ms > 0 ? `Lyrics ${ms} ms später` : `Lyrics ${-ms} ms früher`;
  const signed = (ms: number) => (ms > 0 ? `+${ms}` : ms < 0 ? `−${-ms}` : "0");
  const stat = (s: TapStats) =>
    `Median ${Math.round(s.median_abs_ms)} ms · p95 ${Math.round(s.p95_abs_ms)} ms · Tendenz ${signed(Math.round(s.bias_ms))} ms · ${s.count} Taps`;

  onMount(() => {
    ipc.getClock().then((c) => (offsets = c.offsets));
    ipc.getLyrics().then((l) => (lyrics = l));
    ipc.getCalibration().then((c) => (calibration = c));
    refreshDevices();
    return subscribe(
      on("lyrics", (l) => (lyrics = l)),
      on("calibration", (c) => (calibration = c)),
      on("clock", (c) => {
        const devicesChanged = offsets?.device_ms !== c.offsets.device_ms;
        offsets = c.offsets;
        if (devicesChanged) refreshDevices();
      }),
    );
  });
</script>

<h1>Synchronisation</h1>

<section class="card">
  <h2>Offset dieses Songs</h2>
  {#if now.track}
    <p class="value">
      {now.track.title}: {describe(offsets?.song_ms ?? 0)}
      <button class="inline" onclick={() => ipc.setSongOffset(0)} disabled={!offsets?.song_ms}>Auf 0</button>
    </p>
    <p class="hint">
      Strg+Alt+, (früher) und Strg+Alt+. (später) verschieben nur diesen Song um 100 ms, mit Umschalt um 25 ms.
      LyricHub merkt sich den Wert für diesen Song; der nächste Song startet mit seinem eigenen.
    </p>
  {:else}
    <p class="muted">Spotify spielt gerade nichts.</p>
  {/if}
</section>

<section class="card">
  <h2>Audiogeräte</h2>
  <p class="hint">Dauerhafte Verzögerung der Ausgabe, etwa bei Bluetooth. Neue Bluetooth-Geräte starten mit +150 ms.</p>
  {#if devices.length}
    <table>
      <tbody>
        {#each devices as d (d.id)}
          <tr class:current={d.current}>
            <td>{d.name}{d.current ? " · aktiv" : ""}{d.bluetooth ? " · Bluetooth" : ""}</td>
            <td class="num">
              <input
                class="number"
                type="number"
                step="10"
                value={d.offset_ms}
                onchange={(e) => setDevice(d.id, Number((e.currentTarget as HTMLInputElement).value))}
                aria-label="Offset für {d.name} in ms"
              />
              ms
            </td>
          </tr>
        {/each}
      </tbody>
    </table>
  {:else}
    <p class="muted">Noch kein Ausgabegerät erkannt.</p>
  {/if}
</section>

<section class="card">
  <h2>Kalibrieren</h2>
  <p class="hint">
    Nutzt den laufenden Song. Drück fünfmal die Leertaste genau dann, wenn eine Zeile hörbar beginnt; der Balken im
    Overlay zählt bis zur nächsten Zeile herunter, entscheidend ist aber der Ton. Der Median wird als Offset für das
    aktive Audiogerät vorgeschlagen.
  </p>
  <div class="actions">
    {#if calibration?.active && calibration.mode === "calibrate"}
      <span class="value">Läuft · {calibration.taps.length}/{calibration.needed} · Esc bricht ab</span>
      <button onclick={() => ipc.stopCalibration()}>Abbrechen</button>
    {:else}
      <button class="primary" onclick={() => startMeasure("calibrate")} disabled={!!calibration?.active}>Kalibrierung starten</button>
    {/if}
  </div>
  {#if calibration?.mode === "calibrate" && !calibration.active && calibration.taps.length}
    {#if calibration.suggestion_ms !== null}
      <p class="value">
        Vorschlag: Geräte-Offset {signed(calibration.suggestion_ms)} ms (bisher {signed(calibration.device_offset_ms)} ms)
        {#if calibration.total}<span class="muted"> · {stat(calibration.total)}</span>{/if}
      </p>
      <div class="actions">
        <button class="primary" onclick={applySuggestion} disabled={calibration.suggestion_ms === calibration.device_offset_ms}>Übernehmen</button>
      </div>
    {:else}
      <p class="hint">Zu wenige brauchbare Taps (Abweichungen über 600 ms werden verworfen). Bitte noch einmal.</p>
    {/if}
  {/if}
  {#if calibration?.message}<p class="hint">{calibration.message}</p>{/if}
  {#if startError}<p class="error">{startError}</p>{/if}
</section>

<section class="card">
  <h2>Sync-Protokoll</h2>
  <p class="hint">
    Für die Abnahme (PLAN.md 11.3): Drück über mehrere Songs bei jedem hörbaren Zeilenbeginn die Leertaste. LyricHub misst,
    wie weit das Overlay daneben lag, und schreibt das Ergebnis in den Log-Ordner. Ziel: Median höchstens 100 ms, p95 höchstens
    200 ms.
  </p>
  <div class="actions">
    {#if calibration?.active && calibration.mode === "protocol"}
      <span class="value">Läuft · {calibration.taps.length} Taps{calibration.missed ? ` · ${calibration.missed} ohne Zeile` : ""}</span>
      <button onclick={() => ipc.stopCalibration()}>Messung beenden</button>
    {:else}
      <button onclick={() => startMeasure("protocol")} disabled={!!calibration?.active}>Messung starten</button>
    {/if}
  </div>
  {#if calibration?.mode === "protocol" && calibration.total}
    <p class="value">Gesamt: {stat(calibration.total)} · {Math.round(calibration.total.within_100 * 100)} % innerhalb ±100 ms</p>
    <ul class="songs">
      {#each calibration.per_song as s (s.key)}
        <li>{s.title}: {stat(s.stats)}</li>
      {/each}
    </ul>
    {#if calibration.report_path}<p class="hint mono">{calibration.report_path}</p>{/if}
  {/if}
</section>

<section class="card">
  <h2>Globaler Offset</h2>
  <p class="hint">Gilt für alle Songs und Geräte. Lyrics kommen zu früh? Regler nach rechts. Zu spät? Nach links.</p>
  <div class="offset">
    <span class="end">früher</span>
    <input
      type="range"
      min={-SLIDER_LIMIT}
      max={SLIDER_LIMIT}
      step="10"
      value={Math.max(-SLIDER_LIMIT, Math.min(SLIDER_LIMIT, offset))}
      oninput={(e) => setOffset(Number((e.currentTarget as HTMLInputElement).value))}
      aria-label="Globaler Offset"
    />
    <span class="end">später</span>
    <input
      class="number"
      type="number"
      step="10"
      min={-OFFSET_LIMIT_MS}
      max={OFFSET_LIMIT_MS}
      value={offset}
      onchange={(e) => setOffset(Number((e.currentTarget as HTMLInputElement).value))}
      aria-label="Globaler Offset in Millisekunden"
    />
    <span class="unit">ms</span>
    <button onclick={() => setOffset(0)} disabled={offset === 0}>Auf 0</button>
  </div>
  <p class="value">{describe(offset)}</p>
  {#if offsets}
    <p class="hint">
      Zusammen gerade: global {signed(offsets.global_ms)} + Gerät {signed(offsets.device_ms)} + Song {signed(offsets.song_ms)}
      = {signed(offsets.global_ms + offsets.device_ms + offsets.song_ms)} ms
    </p>
  {/if}
</section>

<section class="card">
  <h2>Lyrics des aktuellen Songs</h2>
  {#if now.track}
    <dl class="kv">
      <dt>Lyrics</dt>
      <dd>{current ? stateText[current.state] : now.track.kind === "podcast" ? "Podcast, keine Suche" : "–"}</dd>
      {#if current?.state === "synced"}
        <dt>Zeilen</dt>
        <dd>{current.lines.filter((l) => l.text).length}</dd>
        <dt>Quelle</dt>
        <dd>{current.source === "cache:lrclib" ? "LRCLIB (aus dem Cache)" : "LRCLIB"}</dd>
      {/if}
    </dl>
    <div class="actions">
      <button onclick={reload} disabled={reloading}>{reloading ? "Wird neu geladen …" : "Lyrics neu laden"}</button>
    </div>
  {:else}
    <p class="muted">Spotify spielt gerade nichts.</p>
  {/if}
  <p class="hint">
    Überblendung (Crossfade) und Automix in Spotify verwischen die Songgrenzen. Für die genaueste Synchronisation in Spotify
    unter Einstellungen → Wiedergabe ausschalten.
  </p>
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
    margin: 6px 0 0;
    color: var(--muted);
    font-size: 12.5px;
  }
  .error {
    color: var(--fail);
  }
  .value {
    margin: 8px 0 0;
    font-weight: 600;
  }
  .offset,
  .actions {
    display: flex;
    align-items: center;
    gap: 10px;
    flex-wrap: wrap;
    margin-top: 12px;
  }
  .offset input[type="range"] {
    width: 320px;
    accent-color: var(--accent);
  }
  .end,
  .unit {
    color: var(--muted);
  }
  .number {
    width: 80px;
    background: #12151b;
    color: var(--text);
    border: 1px solid var(--line);
    border-radius: 7px;
    padding: 6px 8px;
    font: inherit;
  }
  button.inline {
    margin-left: 10px;
    padding: 3px 10px;
    font-weight: 400;
  }
  table {
    width: 100%;
    border-collapse: collapse;
    margin-top: 10px;
  }
  td {
    padding: 6px 4px;
    border-top: 1px solid var(--line);
  }
  td.num {
    text-align: right;
    white-space: nowrap;
    color: var(--muted);
  }
  tr.current td:first-child {
    font-weight: 600;
  }
  .songs {
    margin: 8px 0 0;
    padding-left: 18px;
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
  }
</style>
