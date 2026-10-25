<script lang="ts">
  // Settings → Synchronisation (PLAN.md 9.7 #4, Phase 2 part): global offset with the sign
  // phrased as a sentence (PLAN.md 5.2), the nudge hotkeys, and the lyrics of the current song.
  // Device and song offsets and the calibration assistant follow in Phase 3.
  import { onMount, untrack } from "svelte";
  import { ipc, on, subscribe } from "$lib/shared/ipc";
  import { OFFSET_LIMIT_MS, type LyricsEvent, type NowPlaying, type Settings, type SettingsPatch } from "$lib/shared/types";

  interface Props {
    settings: Settings;
    update: (patch: SettingsPatch) => Promise<void>;
    now: NowPlaying;
  }
  let { settings, update, now }: Props = $props();

  const SLIDER_LIMIT = 2000;
  const PATCH_DELAY_MS = 80;

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

  onMount(() => {
    ipc.getLyrics().then((l) => (lyrics = l));
    return subscribe(on("lyrics", (l) => (lyrics = l)));
  });
</script>

<h1>Synchronisation</h1>

<section class="card">
  <h2>Globaler Offset</h2>
  <p class="hint">
    Lyrics kommen zu früh? Regler nach rechts. Kommen sie zu spät? Nach links. Der Wert gilt für alle Songs; Offsets
    pro Song und pro Audiogerät folgen.
  </p>
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
</section>

<section class="card">
  <h2>Schnell korrigieren, auch im Spiel</h2>
  <dl class="kv">
    <dt>Strg+Alt+,</dt>
    <dd>Lyrics 100 ms früher</dd>
    <dt>Strg+Alt+.</dt>
    <dd>Lyrics 100 ms später</dd>
    <dt>mit Umschalt</dt>
    <dd>in Schritten von 25 ms</dd>
  </dl>
  <p class="hint">Das Overlay zeigt den neuen Wert kurz an. Die Kürzel ändern den globalen Offset.</p>
</section>

<section class="card">
  <h2>Lyrics des aktuellen Songs</h2>
  {#if now.track}
    <dl class="kv">
      <dt>Song</dt>
      <dd>{now.track.title} – {now.track.artist}</dd>
      <dt>Lyrics</dt>
      <dd>{current ? stateText[current.state] : "–"}</dd>
      {#if current?.state === "synced"}
        <dt>Zeilen</dt>
        <dd>{current.lines.filter((l) => l.text).length}</dd>
        <dt>Quelle</dt>
        <dd>{current.source === "cache:lrclib" ? "LRCLIB (aus dem Cache)" : "LRCLIB"}</dd>
        {#if current.lrc_offset_ms}
          <dt>Offset in der LRC-Datei</dt>
          <dd>{current.lrc_offset_ms} ms (bereits berücksichtigt)</dd>
        {/if}
      {/if}
    </dl>
    <div class="actions">
      <button onclick={reload} disabled={reloading}>{reloading ? "Wird neu geladen …" : "Lyrics neu laden"}</button>
    </div>
    <p class="hint">Fragt LRCLIB erneut und ersetzt den Cache-Eintrag, etwa wenn falsche Lyrics angezeigt werden.</p>
  {:else}
    <p class="muted">Spotify spielt gerade nichts.</p>
  {/if}
</section>

<section class="card">
  <h2>Hinweis</h2>
  <p class="hint">
    Überblendung (Crossfade) und Automix in Spotify verwischen die Songgrenzen. Für die genaueste Synchronisation in
    Spotify unter Einstellungen → Wiedergabe ausschalten.
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
  .offset {
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
  .value {
    margin: 8px 0 0;
    font-weight: 600;
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
  .actions {
    margin-top: 12px;
  }
</style>
