<script lang="ts">
  // Small header of the Karaoke and Stack layouts (PLAN.md 9.2): cover 24 px, title – artist,
  // or a toast in its place. Track change (PLAN.md 9.4): the header fades in over 300 ms and the
  // cover grows from 0.96.
  import { songId } from "$lib/shared/ipc";
  import type { TrackInfo } from "$lib/shared/types";

  interface Props {
    track: TrackInfo | null;
    placeholder: { title: string; artist: string };
    toast: string | null;
    /** Small mark for a lyrics lookup that failed on the network (PLAN.md 9.6). */
    offline?: boolean;
  }
  let { track, placeholder, toast, offline = false }: Props = $props();

  const title = $derived(track?.title ?? placeholder.title);
  const artist = $derived(track ? track.artist : placeholder.artist);
</script>

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
        {#if offline}<span class="offline" title="Lyrics nicht erreichbar" aria-label="Lyrics nicht erreichbar"></span>{/if}
      </span>
    {/key}
  {/if}
</header>

<style>
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
  .meta {
    display: flex;
    align-items: center;
    gap: calc(7px * var(--s));
    min-width: 0;
    font-size: calc(13px * var(--s));
    color: var(--text-meta);
    white-space: nowrap;
    animation: fade-in 300ms ease-in-out;
  }
  .cover {
    flex: none;
    width: calc(24px * var(--s));
    height: calc(24px * var(--s));
    border-radius: calc(5px * var(--s));
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
    font-size: calc(13px * var(--s));
    color: var(--text-dim);
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
    color: var(--text);
  }
  .sep {
    flex: none;
    color: var(--text-dim);
  }
  .artist {
    flex: 0 0 auto;
    max-width: 40cqw;
  }
  /* A small hollow dot: the lyrics service is unreachable; details in the settings. */
  .offline {
    flex: none;
    width: calc(7px * var(--s));
    height: calc(7px * var(--s));
    border-radius: 50%;
    border: max(1px, calc(1.5px * var(--s))) solid var(--text-dim);
  }
  .toast {
    padding: 0 calc(10px * var(--s));
    border-radius: 999px;
    background: var(--accent-soft);
    color: var(--accent);
    font-size: calc(13px * var(--s));
    line-height: calc(22px * var(--s));
    font-weight: 600;
    white-space: nowrap;
  }
  @keyframes fade-in {
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
      transform: none;
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .meta,
    .cover {
      animation-duration: 80ms;
    }
  }
</style>
