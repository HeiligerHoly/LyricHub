<script lang="ts">
  // Layout "Karaoke" (PLAN.md 9.2): small header with cover and title – artist, the current line
  // large with the fill, the next line small and dimmed below. Every row has a fixed height, so
  // the window keeps its size while lines change. Line change (PLAN.md 9.4): 220 ms, the new
  // line fades in from 0.55 and rises a little; the fill itself never eases.
  import { songId } from "$lib/shared/ipc";
  import type { Align, Line, TrackInfo } from "$lib/shared/types";
  import type { LocalClock } from "../clock";
  import KaraokeLine from "../KaraokeLine.svelte";
  import { langOf } from "../lyricsTimeline";

  interface Props {
    track: TrackInfo | null;
    placeholder: { title: string; artist: string };
    align: Align;
    active: Line | null;
    /** Changes when the active line changes, for the enter transition. */
    activeKey: string;
    next: Line | null;
    /** What the active row shows when no line is active. */
    status: "loading" | "instrumental" | null;
    hint: string | null;
    toast: string | null;
    clock: LocalClock;
    clockVersion: number;
    staticFill?: number | null;
  }
  let {
    track,
    placeholder,
    align,
    active,
    activeKey,
    next,
    status,
    hint,
    toast,
    clock,
    clockVersion,
    staticFill = null,
  }: Props = $props();

  const title = $derived(track?.title ?? placeholder.title);
  const artist = $derived(track ? track.artist : placeholder.artist);
  const justify = $derived(align === "center" ? "center" : align === "end" ? "flex-end" : "flex-start");
</script>

<div class="karaoke" style="--justify: {justify}">
  <header class="row head">
    {#if toast}
      <span class="toast">{toast}</span>
    {:else}
      {#key songId(track)}
        <span class="meta">
          <span class="cover" aria-hidden="true">
            {#if track?.art}<img src={track.art.url} alt="" draggable="false" />{:else}<span class="placeholder">♪</span>{/if}
          </span>
          <span class="title">{title}</span>
          {#if artist}<span class="sep" aria-hidden="true">–</span><span class="artist">{artist}</span>{/if}
        </span>
      {/key}
    {/if}
  </header>

  <div class="row current">
    {#key activeKey}
      <div class="slot enter">
        {#if active && active.text}
          <KaraokeLine text={active.text} startMs={active.start_ms} fillEndMs={active.fill_end_ms} {clock} {clockVersion} {staticFill} />
        {:else if hint}
          <p class="hint">{hint}</p>
        {:else if status === "loading"}
          <p class="dots" aria-label="Lyrics werden geladen"><span></span><span></span><span></span></p>
        {:else if status === "instrumental"}
          <p class="hint" aria-label="Instrumental">♪</p>
        {/if}
      </div>
    {/key}
  </div>

  <div class="row upcoming">
    {#key next?.start_ms ?? -1}
      {#if next}
        <p class="next enter-next" lang={langOf(next.text)}>{next.text}</p>
      {/if}
    {/key}
  </div>
</div>

<style>
  .karaoke {
    display: grid;
    gap: calc(4px * var(--s));
    --line-size: calc(28px * var(--s));
  }
  .row {
    display: flex;
    justify-content: var(--justify);
    align-items: center;
    min-width: 0;
    overflow: hidden;
  }
  .head {
    height: calc(24px * var(--s));
    /* lets the artist limit refer to the row width (cqw), not to the content-sized meta box */
    container-type: inline-size;
  }
  .current {
    height: calc(var(--line-size) * 1.25);
  }
  .upcoming {
    height: calc(18px * 1.25 * var(--s));
  }
  .slot {
    display: flex;
    justify-content: var(--justify);
    min-width: 0;
    width: 100%;
  }

  .meta {
    display: flex;
    align-items: center;
    gap: calc(7px * var(--s));
    min-width: 0;
    font-size: calc(13px * var(--s));
    color: rgba(255, 255, 255, 0.75);
    white-space: nowrap;
    animation: fade-in 300ms ease-in-out;
  }
  .cover {
    flex: none;
    width: calc(24px * var(--s));
    height: calc(24px * var(--s));
    border-radius: calc(5px * var(--s));
    overflow: hidden;
    background: linear-gradient(135deg, rgba(255, 255, 255, 0.14), rgba(255, 255, 255, 0.04));
  }
  .cover img {
    display: block;
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .placeholder {
    display: grid;
    place-items: center;
    width: 100%;
    height: 100%;
    font-size: calc(13px * var(--s));
    color: rgba(255, 255, 255, 0.6);
  }
  .title,
  .artist {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .title {
    flex: 0 1 auto;
    font-weight: 600;
    color: rgba(255, 255, 255, 0.9);
  }
  .sep {
    flex: none;
    color: rgba(255, 255, 255, 0.45);
  }
  .artist {
    flex: 0 0 auto;
    max-width: 40cqw;
  }
  .toast {
    padding: 0 calc(10px * var(--s));
    border-radius: 999px;
    background: rgba(255, 211, 110, 0.18);
    color: #ffd36e;
    font-size: calc(13px * var(--s));
    line-height: calc(22px * var(--s));
    font-weight: 600;
    white-space: nowrap;
  }

  .next,
  .hint {
    margin: 0;
    min-width: 0;
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
  }
  .next {
    font-size: calc(18px * var(--s));
    line-height: 1.25;
    font-weight: 500;
    color: rgba(255, 255, 255, 0.5);
  }
  .hint {
    font-size: calc(16px * var(--s));
    color: rgba(255, 255, 255, 0.6);
  }

  .dots {
    display: flex;
    gap: calc(6px * var(--s));
    margin: 0;
  }
  .dots span {
    width: calc(7px * var(--s));
    height: calc(7px * var(--s));
    border-radius: 50%;
    background: rgba(255, 255, 255, 0.7);
    animation: pulse 900ms ease-in-out infinite;
  }
  .dots span:nth-child(2) {
    animation-delay: 150ms;
  }
  .dots span:nth-child(3) {
    animation-delay: 300ms;
  }

  .enter {
    animation: line-in 220ms cubic-bezier(0.2, 0.8, 0.2, 1);
  }
  .enter-next {
    animation: next-in 220ms cubic-bezier(0.2, 0.8, 0.2, 1);
  }
  @keyframes line-in {
    from {
      opacity: 0.55;
      transform: translateY(calc(12px * var(--s)));
    }
    to {
      opacity: 1;
      transform: none;
    }
  }
  @keyframes next-in {
    from {
      opacity: 0;
    }
    to {
      opacity: 1;
    }
  }
  @keyframes fade-in {
    from {
      opacity: 0;
    }
    to {
      opacity: 1;
    }
  }
  @keyframes pulse {
    0%,
    100% {
      opacity: 0.25;
    }
    50% {
      opacity: 1;
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .enter,
    .enter-next,
    .meta {
      animation-duration: 80ms;
    }
  }
</style>
