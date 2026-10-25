<script lang="ts">
  // The large row with the active line and its fill, shared by the Karaoke, Stack and Minimal
  // layouts. Without an active line it shows the state (PLAN.md 9.6): loading dots (at most
  // 1.5 s), a hint (4 s) or a note for an instrumental part. Fixed height, so the window keeps
  // its size. Line change (PLAN.md 9.4): 220 ms, the new line fades in from 0.55 and rises a
  // little; the fill itself never eases.
  import type { Line } from "$lib/shared/types";
  import type { LocalClock } from "../clock";
  import KaraokeLine from "../KaraokeLine.svelte";
  import { shownText, shownWords } from "../lyricsTimeline";

  interface Props {
    active: Line | null;
    /** Changes when the active line changes, for the enter transition. */
    activeKey: string;
    romaji: boolean;
    glow: boolean;
    status: "loading" | "instrumental" | null;
    hint: string | null;
    clock: LocalClock;
    clockVersion: number;
    staticFill?: number | null;
  }
  let { active, activeKey, romaji, glow, status, hint, clock, clockVersion, staticFill = null }: Props = $props();
</script>

<div class="row current">
  {#key activeKey}
    <div class="slot enter">
      {#if active && active.text}
        <KaraokeLine
          text={shownText(active, romaji)}
          words={shownWords(active, romaji)}
          {glow}
          startMs={active.start_ms}
          fillEndMs={active.fill_end_ms}
          {clock}
          {clockVersion}
          {staticFill}
        />
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

<style>
  .row {
    display: flex;
    justify-content: var(--justify);
    align-items: center;
    min-width: 0;
    overflow: hidden;
    height: calc(var(--line-size) * 1.25);
  }
  .slot {
    display: flex;
    justify-content: var(--justify);
    min-width: 0;
    width: 100%;
  }
  .hint {
    margin: 0;
    min-width: 0;
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
    font-size: calc(16px * var(--s));
    color: var(--text-meta);
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
    background: var(--text-meta);
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
    .enter {
      animation-duration: 80ms;
    }
  }
</style>
