<script lang="ts">
  import { t } from "#lib/shared/i18n.svelte.js";
  import type { NowPlaying } from "#lib/shared/types.js";

  let { now }: { now: NowPlaying } = $props();
</script>

<section class="np" aria-label={t("np.label")}>
  {#if now.track}
    {#if now.track.art}
      <img class="cover" src={now.track.art.url} alt="" />
    {:else}
      <span class="cover empty" aria-hidden="true">♪</span>
    {/if}
    <div class="text">
      <p class="title" title={now.track.title}>{now.track.title}</p>
      <p class="artist" title={now.track.artist}>{now.track.artist}</p>
      <span class="badge" class:ok={now.status === "playing"}>{t(`np.status.${now.status}`)}</span>
    </div>
  {:else}
    <span class="cover empty" aria-hidden="true">♪</span>
    <div class="text">
      <p class="title">Spotify</p>
      <p class="artist">{t(now.status === "closed" ? "np.status.closed" : "np.idle")}</p>
    </div>
  {/if}
</section>

<style>
  .np {
    display: flex;
    gap: 10px;
    align-items: center;
    padding: 10px;
    border-radius: 10px;
    background: var(--card);
    border: 1px solid var(--line);
  }
  .cover {
    flex: none;
    width: 44px;
    height: 44px;
    border-radius: 6px;
    object-fit: cover;
  }
  .cover.empty {
    display: grid;
    place-items: center;
    background: #262b36;
    color: var(--muted);
  }
  .text {
    min-width: 0;
  }
  .title,
  .artist {
    margin: 0;
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
  }
  .title {
    font-weight: 600;
  }
  .artist {
    color: var(--muted);
    font-size: 13px;
  }
  .badge {
    display: inline-block;
    margin-top: 4px;
    padding: 0 7px;
    border-radius: 999px;
    font-size: 11px;
    background: #2a2f3a;
    color: var(--muted);
  }
  .badge.ok {
    background: rgba(74, 222, 128, 0.15);
    color: var(--ok);
  }
</style>
