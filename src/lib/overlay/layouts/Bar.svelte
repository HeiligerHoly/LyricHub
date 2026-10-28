<script lang="ts">
  // Layout "Bar" (PLAN.md 9.2): one line with cover · title – artist, and from Phase 2 on
  // the current lyric line after a divider (ellipsis, no marquee). Sizes are design sizes at
  // 1080p and scale with --s, which the overlay page sets.
  import { songId } from "#lib/shared/ipc.js";
  import type { Align, TrackInfo } from "#lib/shared/types.js";

  interface Props {
    track: TrackInfo | null;
    /** Shown when there is no track: sample text in the edit mode, a hint when idle. */
    placeholder: { title: string; artist: string };
    lyric?: string | null;
    /** Nudge feedback; replaces title and artist for a moment. */
    toast?: string | null;
    align: Align;
  }
  let { track, placeholder, lyric = null, toast = null, align }: Props = $props();

  const title = $derived(track?.title ?? placeholder.title);
  const artist = $derived(track ? track.artist : placeholder.artist);
  const justify = $derived(align === "center" ? "center" : align === "end" ? "flex-end" : "flex-start");
</script>

{#key songId(track)}
  <div class="bar" style:justify-content={justify}>
    <span class="cover" aria-hidden="true">
      {#if track?.art}
        <img src={track.art.url} alt="" draggable="false" />
      {:else}
        <span class="placeholder">♪</span>
      {/if}
    </span>
    {#if toast}
      <span class="toast">{toast}</span>
    {:else}
      <span class="title" class:with-lyric={!!lyric}>{title}</span>
      {#if artist}
        <span class="sep" aria-hidden="true">–</span>
        <span class="artist" class:with-lyric={!!lyric}>{artist}</span>
      {/if}
    {/if}
    {#if lyric}
      <span class="divider" aria-hidden="true"></span>
      <span class="lyric">{lyric}</span>
    {/if}
  </div>
{/key}

<style>
  .bar {
    display: flex;
    align-items: center;
    gap: calc(10px * var(--s));
    min-width: 0;
    /* Track change (PLAN.md 9.4): 300 ms fade-in; opacity and transform only. */
    animation: track-in 300ms ease-in-out;
  }

  .cover {
    flex: none;
    width: calc(32px * var(--s));
    height: calc(32px * var(--s));
    border-radius: calc(6px * var(--s));
    overflow: hidden;
    background: linear-gradient(135deg, color-mix(in srgb, var(--text) 14%, transparent), color-mix(in srgb, var(--text) 4%, transparent));
    animation: cover-in 300ms ease-in-out;
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
    font-size: calc(17px * var(--s));
    color: var(--text-dim);
  }

  .title,
  .sep,
  .artist,
  .lyric {
    font-size: calc(18px * var(--s));
    line-height: 1.25;
    white-space: nowrap;
  }
  .title,
  .artist,
  .lyric {
    min-width: 0;
    /* Clip with room for the outline and shadows of the themes (theme.ts), which reach past
       the glyphs; too long text still ends in an ellipsis. A fixed length: WebView2 ignores
       calc() with var() here. */
    overflow: clip;
    overflow-clip-margin: 16px;
    text-overflow: ellipsis;
  }
  /* Percentages refer to the fixed plate width. A long title shortens first; the artist
     keeps up to 35 % of the line. With a lyric line both leave it most of the room. */
  .title {
    flex: 0 1 auto;
    font-weight: 650;
    color: var(--text);
  }
  .title.with-lyric {
    max-width: 28%;
  }
  .sep {
    flex: none;
    margin: 0 calc(-4px * var(--s));
    color: var(--text-dim);
  }
  .artist {
    flex: 0 0 auto;
    max-width: 35%;
    font-weight: 450;
    color: var(--text-meta);
  }
  .artist.with-lyric {
    flex-shrink: 1;
    max-width: 18%;
  }
  .toast {
    flex: none;
    padding: 0 calc(10px * var(--s));
    border-radius: 999px;
    background: var(--accent-soft);
    color: var(--accent);
    font-size: calc(15px * var(--s));
    line-height: calc(26px * var(--s));
    font-weight: 600;
    white-space: nowrap;
  }
  .divider {
    flex: none;
    width: max(1px, calc(1.5px * var(--s)));
    height: calc(18px * var(--s));
    background: color-mix(in srgb, var(--text) 30%, transparent);
  }
  .lyric {
    flex: 0 1 auto;
    font-weight: 600;
    color: var(--text);
  }

  @keyframes track-in {
    from {
      opacity: 0;
    }
    to {
      opacity: 1;
    }
  }
  @keyframes cover-in {
    from {
      transform: scale(0.96);
    }
    to {
      transform: scale(1);
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .bar,
    .cover {
      animation-duration: 80ms;
    }
  }
</style>
