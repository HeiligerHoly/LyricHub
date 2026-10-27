<script lang="ts">
  // Settings → Synchronisation (PLAN.md 9.7 #4): global offset, the offset of the current song
  // (set by the nudge hotkeys, kept per song), offsets per audio device, the calibration
  // assistant (5.7) and the tap-based sync protocol (11.3), plus the lyrics of the current song.
  import { onMount, untrack } from "svelte";
  import { t } from "#lib/shared/i18n.svelte.js";
  import { ipc, on, subscribe } from "#lib/shared/ipc.js";
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
  } from "#lib/shared/types.js";

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
  const stateText = (state: LyricsEvent["state"]) => t(`sync.lyrics.state.${state}`);
  async function reload() {
    reloading = true;
    await ipc.reloadLyrics();
    setTimeout(() => (reloading = false), 1500);
  }

  const describe = (ms: number) =>
    ms === 0 ? t("sync.shift.none") : ms > 0 ? t("sync.shift.later", { ms }) : t("sync.shift.earlier", { ms: -ms });
  const signed = (ms: number) => (ms > 0 ? `+${ms}` : ms < 0 ? `−${-ms}` : "0");
  const stat = (s: TapStats) =>
    t("sync.stats", {
      median: Math.round(s.median_abs_ms),
      p95: Math.round(s.p95_abs_ms),
      bias: signed(Math.round(s.bias_ms)),
      count: s.count,
    });

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

<h1>{t("nav.sync")}</h1>

<section class="card">
  <h2>{t("sync.song.title")}</h2>
  {#if now.track}
    <p class="value">
      {now.track.title}: {describe(offsets?.song_ms ?? 0)}
      <button class="inline" onclick={() => ipc.setSongOffset(0)} disabled={!offsets?.song_ms}>{t("sync.toZero")}</button>
    </p>
    <p class="hint">{t("sync.song.hint")}</p>
  {:else}
    <p class="muted">{t("sync.nothingPlaying")}</p>
  {/if}
</section>

<section class="card">
  <h2>{t("sync.devices.title")}</h2>
  <p class="hint">{t("sync.devices.hint")}</p>
  {#if devices.length}
    <table>
      <tbody>
        {#each devices as d (d.id)}
          <tr class:current={d.current}>
            <td>{d.name}{d.current ? ` · ${t("sync.devices.active")}` : ""}{d.bluetooth ? " · Bluetooth" : ""}</td>
            <td class="num">
              <input
                class="number"
                type="number"
                step="10"
                value={d.offset_ms}
                onchange={(e) => setDevice(d.id, Number((e.currentTarget as HTMLInputElement).value))}
                aria-label={t("sync.devices.offset", { name: d.name })}
              />
              ms
            </td>
          </tr>
        {/each}
      </tbody>
    </table>
  {:else}
    <p class="muted">{t("sync.devices.none")}</p>
  {/if}
</section>

<section class="card">
  <h2>{t("sync.calibration.title")}</h2>
  <p class="hint">{t("sync.calibration.hint")}</p>
  <div class="actions">
    {#if calibration?.active && calibration.mode === "calibrate"}
      <span class="value">{t("sync.calibration.running", { n: calibration.taps.length, needed: calibration.needed })}</span>
      <button onclick={() => ipc.stopCalibration()}>{t("common.cancel")}</button>
    {:else}
      <button class="primary" onclick={() => startMeasure("calibrate")} disabled={!!calibration?.active}>{t("sync.calibration.start")}</button>
    {/if}
  </div>
  {#if calibration?.mode === "calibrate" && !calibration.active && calibration.taps.length}
    {#if calibration.suggestion_ms !== null}
      <p class="value">
        {t("sync.calibration.suggestion", { suggested: signed(calibration.suggestion_ms), current: signed(calibration.device_offset_ms) })}
        {#if calibration.total}<span class="muted"> · {stat(calibration.total)}</span>{/if}
      </p>
      <div class="actions">
        <button class="primary" onclick={applySuggestion} disabled={calibration.suggestion_ms === calibration.device_offset_ms}>{t("sync.calibration.apply")}</button>
      </div>
    {:else}
      <p class="hint">{t("sync.calibration.tooFew")}</p>
    {/if}
  {/if}
  {#if calibration?.message}<p class="hint">{calibration.message}</p>{/if}
  {#if startError}<p class="error">{startError}</p>{/if}
</section>

<section class="card">
  <h2>{t("sync.protocol.title")}</h2>
  <p class="hint">{t("sync.protocol.hint")}</p>
  <div class="actions">
    {#if calibration?.active && calibration.mode === "protocol"}
      <span class="value">
        {t("sync.protocol.running", { n: calibration.taps.length })}{calibration.missed ? ` · ${t("sync.protocol.missed", { n: calibration.missed })}` : ""}
      </span>
      <button onclick={() => ipc.stopCalibration()}>{t("sync.protocol.stop")}</button>
    {:else}
      <button onclick={() => startMeasure("protocol")} disabled={!!calibration?.active}>{t("sync.protocol.start")}</button>
    {/if}
  </div>
  {#if calibration?.mode === "protocol" && calibration.total}
    <p class="value">{t("sync.protocol.total", { stats: stat(calibration.total), pct: Math.round(calibration.total.within_100 * 100) })}</p>
    <ul class="songs">
      {#each calibration.per_song as s (s.key)}
        <li>{s.title}: {stat(s.stats)}</li>
      {/each}
    </ul>
    {#if calibration.report_path}<p class="hint mono">{calibration.report_path}</p>{/if}
  {/if}
</section>

<section class="card">
  <h2>{t("sync.global.title")}</h2>
  <p class="hint">{t("sync.global.hint")}</p>
  <div class="offset">
    <span class="end">{t("sync.global.earlier")}</span>
    <input
      type="range"
      min={-SLIDER_LIMIT}
      max={SLIDER_LIMIT}
      step="10"
      value={Math.max(-SLIDER_LIMIT, Math.min(SLIDER_LIMIT, offset))}
      oninput={(e) => setOffset(Number((e.currentTarget as HTMLInputElement).value))}
      aria-label={t("sync.global.title")}
    />
    <span class="end">{t("sync.global.later")}</span>
    <input
      class="number"
      type="number"
      step="10"
      min={-OFFSET_LIMIT_MS}
      max={OFFSET_LIMIT_MS}
      value={offset}
      onchange={(e) => setOffset(Number((e.currentTarget as HTMLInputElement).value))}
      aria-label={t("sync.global.ms")}
    />
    <span class="unit">ms</span>
    <button onclick={() => setOffset(0)} disabled={offset === 0}>{t("sync.toZero")}</button>
  </div>
  <p class="value">{describe(offset)}</p>
  {#if offsets}
    <p class="hint">
      {t("sync.global.sum", {
        global: signed(offsets.global_ms),
        device: signed(offsets.device_ms),
        song: signed(offsets.song_ms),
        total: signed(offsets.global_ms + offsets.device_ms + offsets.song_ms),
      })}
    </p>
  {/if}
</section>

<section class="card">
  <h2>{t("sync.lyrics.title")}</h2>
  {#if now.track}
    <dl class="kv">
      <dt>{t("sync.lyrics.state")}</dt>
      <dd>{current ? stateText(current.state) : now.track.kind === "podcast" ? t("sync.lyrics.podcast") : "–"}</dd>
      {#if current?.state === "synced"}
        <dt>{t("sync.lyrics.lines")}</dt>
        <dd>{current.lines.filter((l) => l.text).length}</dd>
        <dt>{t("sync.lyrics.source")}</dt>
        <dd>{current.source === "cache:lrclib" ? t("sync.lyrics.cached") : "LRCLIB"}</dd>
      {/if}
    </dl>
    <div class="actions">
      <button onclick={reload} disabled={reloading}>{reloading ? t("sync.lyrics.reloading") : t("sync.lyrics.reload")}</button>
    </div>
  {:else}
    <p class="muted">{t("sync.nothingPlaying")}</p>
  {/if}
  <p class="hint">{t("sync.lyrics.crossfade")}</p>
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
