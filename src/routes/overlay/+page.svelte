<script lang="ts">
  // Spike B overlay: static sample content in the planned "Glass Dark" look (PLAN.md 9.3–9.5).
  // The fill animation is a CSS stand-in; Phase 2 drives it from the playback clock.
  import { onMount } from "svelte";
  import { ipc, type OverlayDiagnostics } from "$lib/ipc";

  let diag = $state<OverlayDiagnostics | null>(null);

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
    <p class="line active" lang="de">Dies ist eine Beispielzeile zum Ausrichten</p>
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

  @property --p {
    syntax: "<percentage>";
    inherits: false;
    initial-value: 0%;
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

  .active {
    font-size: 28px;
    font-weight: 650;
    line-height: 1.25;
    /* Karaoke fill: accent up to --p, soft 3 % edge, then the dimmed base colour. */
    background: linear-gradient(
      90deg,
      var(--accent) 0 calc(var(--p) - 1.5%),
      var(--base) calc(var(--p) + 1.5%)
    );
    -webkit-background-clip: text;
    background-clip: text;
    color: transparent;
    animation: fill 5s linear infinite;
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

  @keyframes fill {
    from {
      --p: 0%;
    }
    to {
      --p: 100%;
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .active {
      animation: none;
      --p: 60%;
    }
  }
</style>
