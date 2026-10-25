<script lang="ts">
  // Overlay page (PLAN.md 6, 9). Layouts "Karaoke" and "Bar", lyrics with a line-level fill
  // driven by a local clock, pause dimming, nudge toast and the edit mode. The window is as large
  // as the plate; the page reports the plate size and the backend sizes and places the window
  // (overlay/mod.rs). In the edit mode the window covers the monitor and the plate sits at the
  // draft position.
  //
  // Timing: the backend sends a clock anchor on changes and every 2 s; this page extrapolates
  // (clock.ts), switches lines with one timer per line and lets the compositor run the fill
  // (KaraokeLine.svelte). No IPC and no JavaScript per frame.
  import { onMount, tick } from "svelte";
  import { ipc, on, songId, subscribe } from "$lib/shared/ipc";
  import type {
    ClockEvent,
    EditDraft,
    Line,
    LyricsEvent,
    NudgeEvent,
    OverlayLayout,
    PlaybackStatus,
    TrackInfo,
  } from "$lib/shared/types";
  import { LocalClock } from "./clock";
  import EditFrame from "./EditFrame.svelte";
  import type { EditChange } from "./editMode";
  import Bar from "./layouts/Bar.svelte";
  import Karaoke from "./layouts/Karaoke.svelte";
  import { langOf, lineIndexAt, nextTextLine } from "./lyricsTimeline";

  const SAMPLE_LINE = "Dies ist eine Beispielzeile zum Ausrichten";
  const SAMPLE_NEXT = "次の行はここに少し小さく表示されます";
  const SAMPLE_TRACK = { title: "Titel des Songs", artist: "Artist" };
  const IDLE_TRACK = { title: "LyricHub", artist: "wartet auf Spotify" };
  const DIM_AFTER_MS = 2000;
  const HINT_MS = 3000;
  const REPORT_DEBOUNCE_MS = 50;
  /** Old title comes back if no new one arrives in this time (seek to 0, repeat). */
  const CHANGING_MAX_MS = 700;
  /** PLAN.md 9.6: loading dots at most 1.5 s, state hints 4 s, nudge toast 1.5 s. */
  const LOADING_MAX_MS = 1500;
  const STATE_HINT_MS = 4000;
  const TOAST_MS = 1500;
  /** Room for the soft edge of the fill and a little air next to the longest line. */
  const LINE_EXTRA_PX = 24;

  let track = $state<TrackInfo | null>(null);
  let status = $state<PlaybackStatus>("closed");
  let layout = $state<OverlayLayout | null>(null);
  let hideAfterPauseS = $state(10);
  /** 0 = normal, 1 = dimmed (paused > 2 s), 2 = faded out (paused > hide_after_pause_s) */
  let pauseStage = $state(0);
  let hint = $state(false);
  /** The position jumped to a song start; the old title fades out until the new one arrives. */
  let changing = $state(false);
  let draft = $state<EditChange | null>(null);
  let measured = $state({ w: 0, h: 0 });
  let plate: HTMLElement;

  /** Width the longest line of the current song needs at the current size (Karaoke only). */
  let songLineWidth = $state(0);
  let measurer: HTMLElement;

  let lyrics = $state<LyricsEvent | null>(null);
  const clock = new LocalClock();
  let clockVersion = $state(0);
  let lineIndex = $state(-1);
  let loadingVisible = $state(false);
  let stateHint = $state<string | null>(null);
  let toast = $state<string | null>(null);

  const edit = $derived(layout?.edit ?? null);
  const uiScale = $derived(edit && draft ? draft.scale * edit.res_factor : (layout?.ui_scale ?? 1));
  // A song with long lines widens the plate once, for the whole song, so the box does not
  // change size line by line; the window grows around its anchor. Lines that still do not fit
  // shrink (KaraokeLine), and only beyond that they are cut.
  const widthCss = $derived.by(() => {
    if (edit && draft) return draft.widthCss;
    const base = layout?.width_css ?? 600;
    if (!layout || !songLineWidth) return base;
    const needed = Math.ceil(songLineWidth + 2 * 14 * uiScale + LINE_EXTRA_PX);
    return Math.min(layout.max_width_css, Math.max(base, needed));
  });
  const sample = $derived(!!edit || !!layout?.preview);
  const opacity = $derived.by(() => {
    if (sample) return 1;
    if (!track) return hint ? 0.75 : 0;
    return pauseStage === 0 ? 1 : pauseStage === 1 ? 0.6 : 0;
  });

  const lines = $derived<Line[] | null>(
    lyrics && track && lyrics.key === track.key && lyrics.state === "synced" ? lyrics.lines : null,
  );
  const activeLine = $derived(lines && lineIndex >= 0 ? lines[lineIndex] : null);
  const nextLine = $derived(lines ? nextTextLine(lines, lineIndex) : null);
  const sampleActive: Line = { start_ms: 0, end_ms: 4000, fill_end_ms: 4000, text: SAMPLE_LINE, words: null };
  const sampleNext: Line = { start_ms: 4000, end_ms: null, fill_end_ms: 8000, text: SAMPLE_NEXT, words: null };

  // Pause (PLAN.md 9.4): dim after 2 s, fade out after hide_after_pause_s (0 = never).
  $effect(() => {
    const paused = status === "paused" || status === "stopped";
    const hideMs = hideAfterPauseS > 0 ? Math.max(hideAfterPauseS * 1000, DIM_AFTER_MS) : 0;
    pauseStage = 0;
    if (!paused) return;
    const dim = setTimeout(() => (pauseStage = 1), DIM_AFTER_MS);
    const hide = hideMs ? setTimeout(() => (pauseStage = 2), hideMs) : undefined;
    return () => {
      clearTimeout(dim);
      clearTimeout(hide);
    };
  });

  function afterPaint(fn: () => void) {
    tick().then(() => requestAnimationFrame(() => requestAnimationFrame(fn)));
  }

  // ---- Lines in time ----

  let lineTimer: ReturnType<typeof setTimeout> | undefined;
  /** Picks the line for the current position and sets a timer for the next one. */
  function scheduleLine() {
    clearTimeout(lineTimer);
    if (!lines) {
      lineIndex = -1;
      return;
    }
    const now = performance.now();
    const index = lineIndexAt(lines, clock.display(now));
    lineIndex = index;
    const next = lines[index + 1];
    if (next) {
      const wait = clock.timeUntil(next.start_ms, now);
      if (Number.isFinite(wait)) lineTimer = setTimeout(scheduleLine, wait + 2);
    }
  }

  $effect(() => {
    void lines;
    scheduleLine();
  });

  // Measures every line once per song (and when the size changes) in the active-line font.
  $effect(() => {
    const current = lines;
    void uiScale;
    if (!current || layout?.layout !== "karaoke" || !measurer) {
      songLineWidth = 0;
      return;
    }
    let widest = 0;
    for (const l of current) {
      if (!l.text) continue;
      measurer.lang = langOf(l.text) ?? "";
      measurer.textContent = l.text;
      widest = Math.max(widest, measurer.getBoundingClientRect().width);
    }
    measurer.textContent = "";
    songLineWidth = widest;
  });

  function applyClock(e: ClockEvent) {
    clock.apply(e);
    clockVersion++;
    scheduleLine();
  }

  // ---- Track, lyrics, hints ----

  let changingTimer: ReturnType<typeof setTimeout> | undefined;
  function trackChanging() {
    changing = true;
    clearTimeout(changingTimer);
    changingTimer = setTimeout(() => (changing = false), CHANGING_MAX_MS);
  }

  let lastSong = "";
  function applyTrack(t: TrackInfo | null) {
    track = t;
    const id = songId(t);
    if (id !== lastSong) {
      changing = false;
      if (lyrics && t && lyrics.key !== t.key) lyrics = null;
      stateHint = null;
      loadingVisible = false;
    }
    if (t && id !== lastSong) {
      // Confirms the drawn title to the backend, which logs the track-change latency.
      afterPaint(() => ipc.overlayRendered(t.key));
    }
    lastSong = id;
  }

  let loadingTimer: ReturnType<typeof setTimeout> | undefined;
  let hintStateTimer: ReturnType<typeof setTimeout> | undefined;
  function applyLyrics(e: LyricsEvent | null) {
    if (!e || (track && e.key !== track.key)) return;
    lyrics = e;
    clearTimeout(loadingTimer);
    clearTimeout(hintStateTimer);
    loadingVisible = e.state === "loading";
    if (loadingVisible) loadingTimer = setTimeout(() => (loadingVisible = false), LOADING_MAX_MS);
    stateHint =
      e.state === "missing" || e.state === "unsynced"
        ? "Keine synchronen Lyrics gefunden"
        : e.state === "instrumental"
          ? "Instrumental"
          : null;
    if (stateHint) hintStateTimer = setTimeout(() => (stateHint = null), STATE_HINT_MS);
  }

  let toastTimer: ReturnType<typeof setTimeout> | undefined;
  function showNudge(n: NudgeEvent) {
    const sign = n.offset_ms > 0 ? "+" : n.offset_ms < 0 ? "−" : "±";
    const direction = n.delta_ms > 0 ? "Lyrics später" : "Lyrics früher";
    toast = `Offset ${sign}${Math.abs(n.offset_ms)} ms · ${direction}`;
    clearTimeout(toastTimer);
    toastTimer = setTimeout(() => (toast = null), TOAST_MS);
  }

  // ---- Layout and edit mode ----

  let editSeq = -1;
  let hintTimer: ReturnType<typeof setTimeout> | undefined;
  function applyLayout(l: OverlayLayout) {
    // First layout after start, or the overlay was shown again: brief "waiting" hint if idle.
    const shown = layout === null || l.show_seq !== layout.show_seq;
    layout = l;
    if (l.edit && l.edit.seq !== editSeq) {
      editSeq = l.edit.seq;
      draft = { x: l.edit.x, y: l.edit.y, widthCss: l.width_css, scale: l.edit.scale };
      // Hand the measured box to the backend right away, so Enter saves exact values.
      afterPaint(commitDraft);
    } else if (!l.edit) {
      editSeq = -1;
      draft = null;
    }
    if (shown) {
      hint = true;
      clearTimeout(hintTimer);
      hintTimer = setTimeout(() => (hint = false), HINT_MS);
    }
    afterPaint(() => ipc.overlayLayoutReady(l.seq));
  }

  function currentDraft(): EditDraft | null {
    if (!draft) return null;
    const r = plate.getBoundingClientRect();
    return { x: draft.x, y: draft.y, width: r.width, height: r.height, width_css: draft.widthCss, scale: draft.scale };
  }

  function commitDraft() {
    const d = currentDraft();
    if (d) ipc.overlayEditUpdate(d);
  }

  let reportTimer: ReturnType<typeof setTimeout> | undefined;
  function measure() {
    const r = plate.getBoundingClientRect();
    measured = { w: r.width, h: r.height };
    clearTimeout(reportTimer);
    const scaleAtMeasure = uiScale;
    reportTimer = setTimeout(() => ipc.reportOverlaySize(r.width, r.height, scaleAtMeasure), REPORT_DEBOUNCE_MS);
  }

  onMount(() => {
    const observer = new ResizeObserver(measure);
    observer.observe(plate);

    ipc.getNowPlaying().then((np) => {
      applyTrack(np.track);
      status = np.status;
      ipc.getLyrics().then(applyLyrics);
    });
    ipc.getClock().then(applyClock);
    ipc.getSettings().then((s) => (hideAfterPauseS = s.overlay.hide_after_pause_s));
    ipc.getOverlayLayout().then((l) => l && applyLayout(l));

    const unsubscribe = subscribe(
      on("track", applyTrack),
      on("track-changing", trackChanging),
      on("playback", (p) => (status = p.status)),
      on("clock", applyClock),
      on("lyrics", applyLyrics),
      on("nudge", showNudge),
      on("settings", (s) => (hideAfterPauseS = s.overlay.hide_after_pause_s)),
      on("overlay-layout", applyLayout),
    );
    return () => {
      observer.disconnect();
      unsubscribe();
      clearTimeout(lineTimer);
    };
  });
</script>

<main class="stage" class:edit={!!edit}>
  <div
    class="box"
    style:left="{edit && draft ? draft.x : 0}px"
    style:top="{edit && draft ? draft.y : 0}px"
    style:opacity
  >
    <section class="plate" bind:this={plate} style:width="{widthCss}px" style="--s: {uiScale}">
      <div class="content" class:changing={changing && !sample}>
        {#if layout?.layout === "bar"}
          <Bar
            {track}
            placeholder={sample ? SAMPLE_TRACK : IDLE_TRACK}
            lyric={sample ? SAMPLE_LINE : activeLine?.text || null}
            {toast}
            align={layout?.align ?? "center"}
          />
        {:else}
          <Karaoke
            {track}
            placeholder={sample ? SAMPLE_TRACK : IDLE_TRACK}
            align={layout?.align ?? "center"}
            active={sample ? sampleActive : activeLine}
            activeKey={sample ? "sample" : `${lyrics?.key ?? ""}#${lineIndex}`}
            next={sample ? sampleNext : nextLine}
            status={sample ? null : loadingVisible ? "loading" : activeLine && !activeLine.text ? "instrumental" : null}
            hint={sample ? null : stateHint}
            {toast}
            {clock}
            {clockVersion}
            staticFill={sample ? 0.45 : null}
          />
        {/if}
      </div>
    </section>
  </div>

  <span class="measure" bind:this={measurer} aria-hidden="true" style="--s: {uiScale}"></span>

  {#if edit && draft}
    <EditFrame
      {edit}
      box={{ x: draft.x, y: draft.y, w: measured.w || draft.widthCss, h: measured.h || 48 }}
      scale={draft.scale}
      onchange={(next) => (draft = next)}
      oncommit={() => afterPaint(commitDraft)}
      onsave={() => ipc.saveOverlayGeometry(currentDraft())}
      oncancel={() => ipc.cancelEditMode()}
      onmonitor={() => ipc.overlayEditMonitor(1)}
    />
  {/if}
</main>

<style>
  :global(html),
  :global(body) {
    margin: 0;
    background: transparent;
    overflow: hidden;
    user-select: none;
    -webkit-user-select: none;
  }

  .stage {
    position: fixed;
    inset: 0;
    /* Japanese is the user's third lyrics language (PLAN.md 9.5): JP fonts follow Segoe UI. */
    font-family: "Inter Variable", "Segoe UI Variable", "Segoe UI", "Yu Gothic UI", "Meiryo UI", "Noto Sans JP", sans-serif;
    color: #fff;
  }
  .stage.edit {
    background: rgba(6, 8, 12, 0.35);
  }

  .box {
    position: absolute;
    transition: opacity 400ms ease;
  }

  /* Glass Dark (PLAN.md 9.5). The window is exactly this plate, so it has no outer margin. */
  .plate {
    box-sizing: border-box;
    padding: calc(8px * var(--s)) calc(14px * var(--s));
    border-radius: calc(14px * var(--s));
    background: rgba(12, 12, 16, 0.55);
    text-rendering: optimizeLegibility;
  }

  /* Track change, first half (PLAN.md 9.4): the old title fades as soon as the position
     jumps to the song start; the new one fades in when Spotify publishes it. */
  .content {
    transition: opacity 150ms ease-in-out;
  }
  .content.changing {
    opacity: 0.15;
  }

  /* Same font as the active karaoke line, off screen. */
  .measure {
    position: absolute;
    left: -100000px;
    top: 0;
    visibility: hidden;
    white-space: nowrap;
    font-size: calc(28px * var(--s));
    font-weight: 650;
    line-height: 1.25;
  }
</style>
