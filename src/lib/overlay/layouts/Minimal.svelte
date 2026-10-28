<script lang="ts">
  // Layout "Minimal" (PLAN.md 9.2): only the active line, without a plate, with outline and
  // shadow (theme.ts drops the plate for this layout). A small row above holds toasts and, while
  // no line is active, the title – artist; it has a fixed height so the window keeps its size.
  import type { Align, Line, TrackInfo } from "#lib/shared/types.js";
  import type { LocalClock } from "../clock";
  import ActiveRow from "./ActiveRow.svelte";

  interface Props {
    track: TrackInfo | null;
    placeholder: { title: string; artist: string };
    align: Align;
    active: Line | null;
    activeKey: string;
    romaji: boolean;
    glow?: boolean;
    /** Word-by-word fill; off shows the whole line in the fill colour. */
    fill?: boolean;
    status: "loading" | "instrumental" | null;
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
    romaji,
    glow = true,
    fill = true,
    status,
    toast,
    clock,
    clockVersion,
    staticFill = null,
  }: Props = $props();

  const justify = $derived(align === "center" ? "center" : align === "end" ? "flex-end" : "flex-start");
  const title = $derived(track?.title ?? placeholder.title);
  const artist = $derived(track ? track.artist : placeholder.artist);
  const lineShown = $derived(!!active?.text);
</script>

<div class="minimal" style="--justify: {justify}">
  <div class="row note">
    {#if toast}
      <span class="toast">{toast}</span>
    {:else if !lineShown}
      <span class="title">{title}{artist ? ` – ${artist}` : ""}</span>
    {/if}
  </div>
  <ActiveRow {active} {activeKey} {romaji} {glow} {fill} {status} {clock} {clockVersion} {staticFill} />
</div>

<style>
  .minimal {
    display: grid;
    gap: calc(2px * var(--s));
    --line-size: calc(28px * var(--s));
  }
  .row {
    display: flex;
    justify-content: var(--justify);
    align-items: center;
    min-width: 0;
    overflow: hidden;
  }
  .note {
    height: calc(20px * var(--s));
  }
  .title {
    min-width: 0;
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
    font-size: calc(14px * var(--s));
    font-weight: 600;
    color: var(--text-meta);
    animation: fade-in 300ms ease-in-out;
  }
  .toast {
    padding: 0 calc(10px * var(--s));
    border-radius: 999px;
    background: var(--accent-soft);
    color: var(--accent);
    font-size: calc(13px * var(--s));
    line-height: calc(20px * var(--s));
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
</style>
