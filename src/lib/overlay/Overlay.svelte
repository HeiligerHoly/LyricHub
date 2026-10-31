<script lang="ts">
  // Overlay page (PLAN.md 6, 9). Layouts Karaoke, Bar, Stack and Minimal, themes (theme.ts),
  // lyrics with a word-by-word fill driven by a local clock, pause dimming, toasts and the edit
  // mode. The window is as large
  // as the plate; the page reports the plate size and the backend sizes and places the window
  // (overlay/mod.rs). In the edit mode the window covers the monitor and the plate sits at the
  // draft position. A song without synced lyrics, or a podcast episode, shrinks the plate to
  // a compact card (cover, title, artist); the window keeps its width, so the card sits at the
  // anchor side of the transparent window.
  //
  // Timing: the backend sends a clock anchor on changes and every 2 s; this page extrapolates
  // (clock.ts), switches lines with one timer per line and lets the compositor run the fill
  // (KaraokeLine.svelte). No IPC and no JavaScript per frame.
  import { onMount, tick } from "svelte";
  import { setLanguage, t } from "#lib/shared/i18n.svelte.js";
  import { ipc, on, songId, subscribe } from "#lib/shared/ipc.js";
  import { DEFAULT_PLAYER } from "#lib/shared/types.js";
  import type {
    AppearanceSettings,
    CalibrationView,
    ClockEvent,
    EditDraft,
    Line,
    LyricsEvent,
    NudgeEvent,
    OverlayLayout,
    PlaybackStatus,
    Settings,
    TrackInfo,
  } from "#lib/shared/types.js";
  import { coverAccent } from "./accent";
  import { LocalClock } from "./clock";
  import type { Rgb } from "./color";
  import EditFrame from "./EditFrame.svelte";
  import type { EditChange } from "./editMode";
  import Bar from "./layouts/Bar.svelte";
  import Compact from "./layouts/Compact.svelte";
  import Karaoke from "./layouts/Karaoke.svelte";
  import Minimal from "./layouts/Minimal.svelte";
  import Stack from "./layouts/Stack.svelte";
  import { langOf, lineIndexAt, nextTextLine, nextTextLines, previousTextLine, shownText } from "./lyricsTimeline";
  import { baseAccent, contrastGround, cssVars, look } from "./theme";

  // Sample lines (invented text) live in the locales; the Japanese next line stays the same in
  // every language.
  const SAMPLE_NEXT = "次の歌詞はここに少し小さく表示されます";
  /** What the backend makes of SAMPLE_NEXT (test `overlay_sample_line` in lyrics/romaji.rs). */
  const SAMPLE_NEXT_ROMAJI = "tsugi no kashi wa koko ni sukoshi chiisaku hyōji saremasu";
  const DIM_AFTER_MS = 2000;
  const HINT_MS = 3000;
  const REPORT_DEBOUNCE_MS = 50;
  /** Old title comes back if no new one arrives in this time (seek to 0, repeat). */
  const CHANGING_MAX_MS = 700;
  /** PLAN.md 9.6: loading dots at most 1.5 s, nudge toast 1.5 s. */
  const LOADING_MAX_MS = 1500;
  const TOAST_MS = 1500;
  const NOTICE_MS = 5000;
  /** Room for the soft edge of the fill and a little air next to the longest line. */
  const LINE_EXTRA_PX = 24;

  let track = $state<TrackInfo | null>(null);
  let status = $state<PlaybackStatus>("closed");
  let layout = $state<OverlayLayout | null>(null);
  let hideAfterPauseS = $state(10);
  let appearance = $state<AppearanceSettings | null>(null);
  /** A browser is the chosen player (PLAN.md 13.4); idle texts then do not name Spotify. */
  let browserPlayer = $state(false);
  /** Japanese lines in romaji; the backend sends the romaji with the lyrics. */
  let romaji = $state(false);
  /** Windows "show animations" off. */
  let osReducedMotion = $state(false);
  /** Accent picked from the current cover; null until known or without a usable colour. */
  let coverRgb = $state<Rgb | null>(null);
  /** 0 = normal, 1 = dimmed (paused > 2 s), 2 = faded out (paused > hide_after_pause_s) */
  let pauseStage = $state(0);
  let hint = $state(false);
  /** The position jumped to a song start; the old title fades out until the new one arrives. */
  let changing = $state(false);
  let draft = $state<EditChange | null>(null);
  let measured = $state({ w: 0, h: 0 });
  let plate: HTMLElement;

  /** Width the longest line of the current song needs at the current size (not for Bar). */
  let songLineWidth = $state(0);
  let measurer: HTMLElement;

  let lyrics = $state<LyricsEvent | null>(null);
  const clock = new LocalClock();
  let clockVersion = $state(0);
  let lineIndex = $state(-1);
  let loadingVisible = $state(false);
  /** The lyrics of the song are known and none are synced. Kept over a track change until the
   *  next song's lyrics are known, so a run of such songs does not grow and shrink each time. */
  let noLyrics = $state(false);
  let toast = $state<string | null>(null);
  let calibration = $state<CalibrationView | null>(null);
  /** Countdown to the next line start while calibrating: restarts per line. */
  let countdown = $state<{ key: number; ms: number } | null>(null);

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
  /** Where a plate narrower than the window sits: on the side of the anchor. */
  const justify = $derived.by(() => {
    const align = layout?.align ?? "center";
    return align === "center" ? "center" : align === "end" ? "flex-end" : "flex-start";
  });
  const opacity = $derived.by(() => {
    if (sample) return 1;
    if (!track) return hint ? 0.75 : 0;
    // Spotify Free ads stay hidden (PLAN.md 5.5).
    if (track.kind === "ad") return 0;
    return pauseStage === 0 ? 1 : pauseStage === 1 ? 0.6 : 0;
  });
  const theme = $derived(appearance ? look(appearance, layout?.layout) : null);
  const reduced = $derived(osReducedMotion || !!appearance?.reduced_motion);
  const accentRgb = $derived.by<Rgb>(() => {
    if (!appearance || !theme) return [255, 211, 110];
    if (appearance.accent === "cover" && theme.accentAllowed && coverRgb) return coverRgb;
    return baseAccent(appearance, theme);
  });
  const stageStyle = $derived(theme && appearance ? cssVars(theme, accentRgb, appearance.font) : "");
  // Accent from the cover (PLAN.md 9.5), once per cover and plate colour.
  $effect(() => {
    const url = track?.art?.url;
    const t = theme;
    if (!url || !t || appearance?.accent !== "cover" || !t.accentAllowed) {
      coverRgb = null;
      return;
    }
    let current = true;
    coverAccent(url, contrastGround(t)).then((c) => {
      if (current) coverRgb = c;
    });
    return () => {
      current = false;
    };
  });

  const measuring = $derived(calibration?.active ? calibration : null);
  const headerNote = $derived.by(() => {
    if (toast) return toast;
    if (!measuring) return null;
    const n = measuring.taps.length;
    return measuring.mode === "calibrate"
      ? t("overlay.calibrate", { n, needed: measuring.needed })
      : t("overlay.protocol", { n });
  });

  const lines = $derived<Line[] | null>(
    lyrics && track && lyrics.key === track.key && lyrics.state === "synced" ? lyrics.lines : null,
  );
  const activeLine = $derived(lines && lineIndex >= 0 ? lines[lineIndex] : null);
  const nextLine = $derived(lines ? nextTextLine(lines, lineIndex) : null);
  // Stack: the line before and two after the active one.
  const previousLine = $derived(lines && layout?.layout === "stack" ? previousTextLine(lines, lineIndex) : null);
  const upcomingLines = $derived(lines && layout?.layout === "stack" ? nextTextLines(lines, lineIndex, 2) : []);
  /** The lyrics service could not be reached (PLAN.md 9.6: a small mark, no text in the game). */
  const offline = $derived(!!lyrics && !!track && lyrics.key === track.key && lyrics.state === "error");
  // Lyrics recognized from the singing by a plugin (PLAN-plugins-2.md 4.1), or stretched from the
  // original of a sped-up or slowed version (IDEEN.md, package B), carry a small mark.
  const mark = $derived.by(() => {
    if (!lyrics || !track || lyrics.key !== track.key) return null;
    if (lyrics.source.endsWith(":recognized")) return t("overlay.recognized");
    if (lyrics.source.endsWith("lrclib:stretched")) return t("overlay.stretched");
    return null;
  });
  /** Nothing to sing along to: the compact card instead of the layout. Podcast episodes and
   *  browser videos get no lyrics lookup at all (PLAN.md 5.5, 13.4). */
  const compact = $derived(
    !sample && !measuring && !!track && (noLyrics || track.kind === "podcast" || track.kind === "video"),
  );
  const rowStatus = $derived(sample ? null : loadingVisible ? ("loading" as const) : activeLine && !activeLine.text ? ("instrumental" as const) : null);
  // Derived, so the edit mode and the preview follow a language change.
  const sampleTrack = $derived({ title: t("overlay.sample.title"), artist: t("overlay.sample.artist") });
  const idleTrack = $derived({ title: "LyricHub", artist: t(browserPlayer ? "overlay.idleArtistPlayer" : "overlay.idleArtist") });
  const sampleActive = $derived<Line>({
    start_ms: 0,
    end_ms: 4000,
    fill_end_ms: 4000,
    text: t("overlay.sample.line"),
    words: null,
    romaji: null,
    romaji_words: null,
  });
  const sampleLine = (text: string, start_ms: number): Line => ({ start_ms, end_ms: null, fill_end_ms: start_ms + 3000, text, words: null, romaji: null, romaji_words: null });
  /** Stack preview: one line before, one more after. */
  const samplePrevious = $derived(sampleLine(t("overlay.sample.previous"), -4000));
  const sampleLater = $derived(sampleLine(t("overlay.sample.later"), 8000));
  const sampleNext: Line = {
    start_ms: 4000,
    end_ms: null,
    fill_end_ms: 8000,
    text: SAMPLE_NEXT,
    words: null,
    romaji: SAMPLE_NEXT_ROMAJI,
    romaji_words: null,
  };

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
    const upcoming = nextTextLine(lines, index);
    const until = upcoming ? clock.timeUntil(upcoming.start_ms, now) : Infinity;
    countdown =
      measuring?.mode === "calibrate" && upcoming && Number.isFinite(until) ? { key: upcoming.start_ms, ms: until } : null;
  }

  $effect(() => {
    void measuring;
    scheduleLine();
  });

  $effect(() => {
    void lines;
    scheduleLine();
  });

  // Measures every line once per song (and when the size or the script changes) in the
  // active-line font.
  $effect(() => {
    const current = lines;
    const inRomaji = romaji;
    void uiScale;
    if (!current || !layout || layout.layout === "bar" || !measurer) {
      songLineWidth = 0;
      return;
    }
    let widest = 0;
    for (const l of current) {
      if (!l.text) continue;
      const text = shownText(l, inRomaji);
      measurer.lang = langOf(text) ?? "";
      measurer.textContent = text;
      widest = Math.max(widest, measurer.getBoundingClientRect().width);
    }
    measurer.textContent = "";
    songLineWidth = widest;
  });

  function applyClock(e: ClockEvent) {
    clock.apply(e);
    clockVersion++;
    scheduleLine();
    // Lets the backend log the time from Spotify's report to the corrected display.
    if (e.kind === "seek") afterPaint(() => ipc.clockApplied(e.seq));
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
    if (!t) noLyrics = false;
    const id = songId(t);
    if (id !== lastSong) {
      changing = false;
      if (lyrics && t && lyrics.key !== t.key) lyrics = null;
      loadingVisible = false;
    }
    if (t && id !== lastSong) {
      // Confirms the drawn title to the backend, which logs the track-change latency.
      afterPaint(() => ipc.overlayRendered(t.key));
    }
    lastSong = id;
  }

  let loadingTimer: ReturnType<typeof setTimeout> | undefined;
  function applyLyrics(e: LyricsEvent | null) {
    if (!e || (track && e.key !== track.key)) return;
    lyrics = e;
    clearTimeout(loadingTimer);
    loadingVisible = e.state === "loading";
    if (loadingVisible) loadingTimer = setTimeout(() => (loadingVisible = false), LOADING_MAX_MS);
    // Unsynced lyrics are not shown either; "error" means the service could not be reached.
    if (e.state !== "loading") noLyrics = e.state !== "synced";
  }

  let settingsSeen = false;
  function applySettings(s: Settings) {
    // First, so a toast below already speaks the new language.
    setLanguage(s.behavior.language);
    hideAfterPauseS = s.overlay.hide_after_pause_s;
    appearance = s.appearance;
    browserPlayer = s.player.app_id !== DEFAULT_PLAYER;
    // Feedback for Ctrl+Alt+R, which changes nothing visible while a non-Japanese song plays.
    if (settingsSeen && s.appearance.romaji !== romaji) showToast(t(s.appearance.romaji ? "overlay.toast.romajiOn" : "overlay.toast.romajiOff"));
    romaji = s.appearance.romaji;
    settingsSeen = true;
  }

  let toastTimer: ReturnType<typeof setTimeout> | undefined;
  function showToast(text: string, ms = TOAST_MS) {
    toast = text;
    clearTimeout(toastTimer);
    toastTimer = setTimeout(() => (toast = null), ms);
  }

  function showNudge(n: NudgeEvent) {
    const sign = n.offset_ms > 0 ? "+" : n.offset_ms < 0 ? "−" : "±";
    const direction = t(n.delta_ms > 0 ? "overlay.toast.later" : "overlay.toast.earlier");
    showToast(t("overlay.toast.songOffset", { sign, ms: Math.abs(n.offset_ms), direction }));
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
    const motionQuery = matchMedia("(prefers-reduced-motion: reduce)");
    osReducedMotion = motionQuery.matches;
    const onMotion = () => (osReducedMotion = motionQuery.matches);
    motionQuery.addEventListener("change", onMotion);

    ipc.getNowPlaying().then((np) => {
      applyTrack(np.track);
      status = np.status;
      ipc.getLyrics().then(applyLyrics);
    });
    ipc.getClock().then(applyClock);
    ipc.getSettings().then(applySettings);
    ipc.getOverlayLayout().then((l) => l && applyLayout(l));
    ipc.getCalibration().then((c) => (calibration = c));

    const unsubscribe = subscribe(
      on("track", applyTrack),
      on("track-changing", trackChanging),
      on("playback", (p) => (status = p.status)),
      on("clock", applyClock),
      on("lyrics", applyLyrics),
      on("nudge", showNudge),
      on("notice", (n) => showToast(n.text, NOTICE_MS)),
      on("calibration", (c) => (calibration = c)),
      on("settings", applySettings),
      on("overlay-layout", applyLayout),
    );
    return () => {
      observer.disconnect();
      motionQuery.removeEventListener("change", onMotion);
      unsubscribe();
      clearTimeout(lineTimer);
    };
  });
</script>

<!-- --s also on the stage: the theme variables (outline, shadows) are computed here. -->
<main class="stage" class:edit={!!edit} class:reduced style="{stageStyle}; --s: {uiScale}">
  <div
    class="box"
    class:spread={!edit}
    style:left="{edit && draft ? draft.x : 0}px"
    style:top="{edit && draft ? draft.y : 0}px"
    style:justify-content={justify}
    style:opacity
  >
    <section
      class="plate"
      class:compact
      bind:this={plate}
      style:width={compact ? null : `${widthCss}px`}
      style:max-width="{widthCss}px"
      style="--s: {uiScale}"
    >
      <div class="content" class:changing={changing && !sample}>
        {#if compact && track && layout?.layout !== "bar"}
          <Compact {track} toast={headerNote} {offline} />
        {:else if layout?.layout === "bar"}
          <Bar
            {track}
            placeholder={sample ? sampleTrack : idleTrack}
            lyric={sample ? sampleActive.text : activeLine?.text ? shownText(activeLine, romaji) : null}
            toast={headerNote}
            align={layout?.align ?? "center"}
          />
        {:else if layout?.layout === "stack"}
          <Stack
            {track}
            placeholder={sample ? sampleTrack : idleTrack}
            align={layout?.align ?? "center"}
            previous={sample ? samplePrevious : previousLine}
            active={sample ? sampleActive : activeLine}
            activeKey={sample ? "sample" : `${lyrics?.key ?? ""}#${lineIndex}`}
            upcoming={sample ? [sampleNext, sampleLater] : upcomingLines}
            {romaji}
            glow={!reduced}
            fill={appearance?.word_fill ?? true}
            status={rowStatus}
            toast={headerNote}
            {offline}
            {mark}
            {clock}
            {clockVersion}
            staticFill={sample ? 0.45 : null}
          />
        {:else if layout?.layout === "minimal"}
          <Minimal
            {track}
            placeholder={sample ? sampleTrack : idleTrack}
            align={layout?.align ?? "center"}
            active={sample ? sampleActive : activeLine}
            activeKey={sample ? "sample" : `${lyrics?.key ?? ""}#${lineIndex}`}
            {romaji}
            glow={!reduced}
            fill={appearance?.word_fill ?? true}
            status={rowStatus}
            toast={headerNote}
            {clock}
            {clockVersion}
            staticFill={sample ? 0.45 : null}
          />
        {:else}
          <Karaoke
            {track}
            placeholder={sample ? sampleTrack : idleTrack}
            align={layout?.align ?? "center"}
            active={sample ? sampleActive : activeLine}
            activeKey={sample ? "sample" : `${lyrics?.key ?? ""}#${lineIndex}`}
            next={sample ? sampleNext : nextLine}
            {romaji}
            glow={!reduced}
            fill={appearance?.word_fill ?? true}
            status={rowStatus}
            toast={headerNote}
            {offline}
            {mark}
            countdown={sample ? null : countdown}
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
    /* Japanese is the user's third lyrics language (PLAN.md 9.5): JP fonts follow Segoe UI.
       --font is the font picked in the settings; Inter is bundled. */
    font-family: var(--font, "Inter Variable"), "Inter Variable", "Segoe UI Variable", "Segoe UI", "Yu Gothic UI", "Meiryo UI", "Noto Sans JP", sans-serif;
    color: var(--text, #fff);
    /* Outline for plate-less themes and the Minimal layout (PLAN.md 9.3); 0 px otherwise. */
    -webkit-text-stroke: var(--stroke, 0px) var(--stroke-color, transparent);
    paint-order: stroke fill;
    text-shadow: var(--text-shadow, none);
  }
  /* Reduced motion (PLAN.md 9.4): every transition and animation at most 80 ms, no pulsing. */
  .stage.reduced :global(*) {
    transition-duration: 80ms !important;
    animation-duration: 80ms !important;
  }
  .stage.edit {
    background: rgba(6, 8, 12, 0.35);
  }

  .box {
    position: absolute;
    transition: opacity 400ms ease;
  }
  /* Outside the edit mode the box spans the window, so a compact plate can sit on the anchor
     side (justify-content). */
  .box.spread {
    right: 0;
    display: flex;
  }

  /* Theme plate (theme.ts). The window is exactly this plate, so it has no outer margin. */
  .plate {
    /* A full plate keeps its width even before the window has grown to it; it is measured. */
    flex: none;
    box-sizing: border-box;
    padding: calc(8px * var(--s)) calc(14px * var(--s));
    border-radius: calc(var(--plate-radius, 14px) * var(--s));
    background: var(--plate-bg, rgba(12, 12, 16, 0.55));
    text-rendering: optimizeLegibility;
  }
  /* As wide as cover, title and artist need, at most the normal width; long titles end in an
     ellipsis. */
  .plate.compact {
    flex: 0 1 auto;
    min-width: 0;
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
