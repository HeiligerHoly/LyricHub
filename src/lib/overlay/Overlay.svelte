<script lang="ts">
  // Overlay page (PLAN.md 6, 9). Phase 1: layout "Bar" with cover, title and artist, pause
  // dimming, and the edit mode. The window is as large as the plate; the page reports the
  // plate size and the backend sizes and places the window (overlay/mod.rs). In the edit mode
  // the window covers the monitor and the plate sits at the draft position.
  //
  // No animation runs continuously: transitions only on track change, pause and edit mode.
  import { onMount, tick } from "svelte";
  import { ipc, on, songId, subscribe } from "$lib/shared/ipc";
  import type { EditDraft, OverlayLayout, PlaybackStatus, TrackInfo } from "$lib/shared/types";
  import Bar from "./layouts/Bar.svelte";
  import EditFrame from "./EditFrame.svelte";
  import type { EditChange } from "./editMode";

  const SAMPLE_LINE = "Dies ist eine Beispielzeile zum Ausrichten";
  const SAMPLE_TRACK = { title: "Titel des Songs", artist: "Artist" };
  const IDLE_TRACK = { title: "LyricHub", artist: "wartet auf Spotify" };
  const DIM_AFTER_MS = 2000;
  const HINT_MS = 3000;
  const REPORT_DEBOUNCE_MS = 50;
  /** Old title comes back if no new one arrives in this time (seek to 0, repeat). */
  const CHANGING_MAX_MS = 700;

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

  const edit = $derived(layout?.edit ?? null);
  const widthCss = $derived(edit && draft ? draft.widthCss : (layout?.width_css ?? 600));
  const uiScale = $derived(edit && draft ? draft.scale * edit.res_factor : (layout?.ui_scale ?? 1));
  const sample = $derived(!!edit || !!layout?.preview);
  const opacity = $derived.by(() => {
    if (sample) return 1;
    if (!track) return hint ? 0.75 : 0;
    return pauseStage === 0 ? 1 : pauseStage === 1 ? 0.6 : 0;
  });

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
    if (id !== lastSong) changing = false;
    if (t && id !== lastSong) {
      // Confirms the drawn title to the backend, which logs the track-change latency.
      afterPaint(() => ipc.overlayRendered(t.key));
    }
    lastSong = id;
  }

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
    });
    ipc.getSettings().then((s) => (hideAfterPauseS = s.overlay.hide_after_pause_s));
    ipc.getOverlayLayout().then((l) => l && applyLayout(l));

    const unsubscribe = subscribe(
      on("track", applyTrack),
      on("track-changing", trackChanging),
      on("playback", (p) => (status = p.status)),
      on("settings", (s) => (hideAfterPauseS = s.overlay.hide_after_pause_s)),
      on("overlay-layout", applyLayout),
    );
    return () => {
      observer.disconnect();
      unsubscribe();
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
        <Bar
          {track}
          placeholder={sample ? SAMPLE_TRACK : IDLE_TRACK}
          lyric={sample ? SAMPLE_LINE : null}
          align={layout?.align ?? "center"}
        />
      </div>
    </section>
  </div>

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
</style>
