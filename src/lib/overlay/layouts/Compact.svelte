<script lang="ts">
  // Compact card for a song without synced lyrics and for podcast episodes: cover, title and
  // artist, nothing else, so the plate takes as little room in the game as it can. It replaces
  // the Karaoke, Stack and Minimal layouts while there is nothing to sing along to; Bar keeps
  // its own row. A toast takes the place of title and artist for a moment.
  import { t } from "#lib/shared/i18n.svelte.js";
  import { songId } from "#lib/shared/ipc.js";
  import type { TrackInfo } from "#lib/shared/types.js";

  interface Props {
    track: TrackInfo;
    toast: string | null;
    /** Small mark for a lyrics lookup that failed on the network (PLAN.md 9.6). */
    offline?: boolean;
  }
  let { track, toast, offline = false }: Props = $props();
</script>

{#key songId(track)}
  <div class="compact">
    <span class="cover" aria-hidden="true">
      {#if track.art}<img src={track.art.url} alt="" draggable="false" />{:else}<span class="placeholder">♪</span>{/if}
    </span>
    {#if toast}
      <span class="toast">{toast}</span>
    {:else}
      <span class="text">
        <span class="title">{track.title}</span>
        {#if track.artist || offline}
          <span class="artist">
            <span class="name">{track.artist}</span>
            {#if offline}<span class="offline" title={t("overlay.offline")} aria-label={t("overlay.offline")}></span>{/if}
          </span>
        {/if}
      </span>
    {/if}
  </div>
{/key}

<style>
  .compact {
    display: flex;
    align-items: center;
    gap: calc(10px * var(--s));
    min-width: 0;
    /* Track change (PLAN.md 9.4): 300 ms fade-in; opacity and transform only. */
    animation: fade-in 300ms ease-in-out;
  }
  .cover {
    flex: none;
    width: calc(40px * var(--s));
    height: calc(40px * var(--s));
    border-radius: calc(7px * var(--s));
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
    font-size: calc(18px * var(--s));
    color: var(--text-dim);
  }
  .text {
    display: grid;
    min-width: 0;
    line-height: 1.25;
    white-space: nowrap;
  }
  .title,
  .name {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .title {
    font-size: calc(16px * var(--s));
    font-weight: 650;
    color: var(--text);
  }
  .artist {
    display: flex;
    align-items: center;
    gap: calc(7px * var(--s));
    min-width: 0;
    font-size: calc(13px * var(--s));
    color: var(--text-meta);
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
    .compact,
    .cover {
      animation-duration: 80ms;
    }
  }
</style>
