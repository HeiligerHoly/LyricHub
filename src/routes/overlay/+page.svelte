<script lang="ts">
  // Spike B overlay: static sample content in the planned "Glass Dark" look (PLAN.md 9.3–9.5).
  // The fill animation is a CSS stand-in; Phase 2 drives it from the playback clock.
  import { onMount } from "svelte";
  import { ipc, type OverlayDiagnostics } from "$lib/ipc";

  let diag = $state<OverlayDiagnostics | null>(null);
  const sampleLine = "Dies ist eine Beispielzeile zum Ausrichten";

  onMount(() => {
    const refresh = async () => {
      try {
        diag = await ipc.overlayDiagnostics();
      } catch {
        diag = null;
      }
    };
    refresh();
    // One IPC call per second is fine for a diagnostics line; never per frame.
    const id = setInterval(refresh, 1000);
    return () => clearInterval(id);
  });
</script>

<main class="overlay">
  <section class="panel">
    <header class="meta">
      <span class="cover" aria-hidden="true"></span>
      <span class="track">LyricHub · Overlay-Test</span>
    </header>
    <p class="line active" lang="de">
      <span class="base">{sampleLine}</span>
      <span class="reveal" aria-hidden="true"><span class="fill">{sampleLine}</span></span>
    </p>
    <p class="line next" lang="ja">次の行はここに少し小さく表示されます</p>
    <footer class="diag">
      {#if diag}
        {diag.click_through ? "klickdurchlässig" : "Edit-Modus: fängt Klicks"} ·
        {diag.topmost ? "topmost" : "nicht topmost"} ·
        {diag.no_activate ? "no-activate" : "aktivierbar"} ·
        {diag.tool_window ? "nicht in Alt+Tab" : "in Alt+Tab"}
      {:else}
        Diagnose nicht verfügbar
      {/if}
    </footer>
  </section>
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

  .overlay {
    --accent: #ffd36e;
    --base: rgba(255, 255, 255, 0.55);
    --plate: rgba(12, 12, 16, 0.55);
    box-sizing: border-box;
    width: 100vw;
    height: 100vh;
    padding: 8px;
    font-family: "Inter Variable", "Segoe UI Variable", "Segoe UI", "Yu Gothic UI", "Meiryo UI", sans-serif;
    color: #fff;
  }

  .panel {
    box-sizing: border-box;
    height: 100%;
    padding: 12px 22px;
    border-radius: 14px;
    background: var(--plate);
    display: flex;
    flex-direction: column;
    justify-content: center;
    gap: 4px;
  }

  .meta {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 13px;
    color: rgba(255, 255, 255, 0.7);
  }

  .cover {
    width: 20px;
    height: 20px;
    border-radius: 4px;
    background: linear-gradient(135deg, #7c5cff, #ff5c8a);
  }

  .line {
    margin: 0;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  /* Karaoke fill without repaints: the accent copy sits in a clipping layer. The layer slides
     in from the left while its content slides the opposite way, so the text stays still and
     only the visible part grows. Both moves are transforms the compositor handles alone.
     (An animated gradient with background-clip: text repainted every frame: ~55 % of a core.) */
  .active {
    position: relative;
    width: fit-content; /* the fill must span the text, not the whole panel */
    max-width: 100%;
    font-size: 28px;
    font-weight: 650;
    line-height: 1.25;
  }
  .base {
    color: var(--base);
  }
  .reveal {
    position: absolute;
    inset: 0;
    overflow: hidden;
    /* soft leading edge that travels with the fill */
    mask-image: linear-gradient(90deg, #000 calc(100% - 18px), transparent);
    will-change: transform;
    /* steps(150) over 5 s = 30 updates/s: on a 164 Hz monitor the compositor otherwise
       produces 164 frames/s for a fill nobody can tell apart at 30. */
    animation: reveal-outer 5s steps(150, jump-none) infinite;
  }
  .fill {
    display: block;
    color: var(--accent);
    white-space: nowrap;
    will-change: transform;
    animation: reveal-inner 5s steps(150, jump-none) infinite;
  }
  @keyframes reveal-outer {
    from {
      transform: translateX(-100%);
    }
    to {
      transform: translateX(0);
    }
  }
  @keyframes reveal-inner {
    from {
      transform: translateX(100%);
    }
    to {
      transform: translateX(0);
    }
  }

  .next {
    font-size: 18px;
    font-weight: 500;
    color: rgba(255, 255, 255, 0.5);
    line-break: strict;
  }

  .diag {
    margin-top: 2px;
    font-size: 11px;
    color: rgba(255, 255, 255, 0.45);
  }

  @media (prefers-reduced-motion: reduce) {
    .reveal,
    .fill {
      animation: none;
      transform: none;
    }
  }
</style>
