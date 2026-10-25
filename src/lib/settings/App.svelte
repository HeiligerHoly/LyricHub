<script lang="ts">
  // Settings window (PLAN.md 9.7): navigation left, page right, dark theme like the overlay.
  // Pages: Overlay, Darstellung, Synchronisation, Verhalten, Diagnose. The window exists only
  // while it is open (settings_window.rs).
  import { onMount } from "svelte";
  import { ipc, on, subscribe } from "$lib/shared/ipc";
  import type { NowPlaying, Settings, SettingsPatch } from "$lib/shared/types";
  import NowPlayingCard from "./NowPlayingCard.svelte";
  import OverlayPage from "./pages/OverlayPage.svelte";
  import AppearancePage from "./pages/AppearancePage.svelte";
  import BehaviorPage from "./pages/BehaviorPage.svelte";
  import DiagnosticsPage from "./pages/DiagnosticsPage.svelte";
  import SyncPage from "./pages/SyncPage.svelte";

  type PageId = "overlay" | "appearance" | "sync" | "behavior" | "diagnostics";
  const pages: { id: PageId; label: string }[] = [
    { id: "overlay", label: "Overlay" },
    { id: "appearance", label: "Darstellung" },
    { id: "sync", label: "Synchronisation" },
    { id: "behavior", label: "Verhalten" },
    { id: "diagnostics", label: "Diagnose" },
  ];

  let page = $state<PageId>("overlay");
  let settings = $state<Settings | null>(null);
  let now = $state<NowPlaying>({ track: null, status: "closed" });
  let editing = $state(false);
  let version = $state("");
  let error = $state<string | null>(null);

  async function update(patch: SettingsPatch) {
    try {
      settings = await ipc.setSettings(patch);
      error = null;
    } catch (e) {
      error = String(e);
    }
  }

  onMount(() => {
    ipc.getSettings().then((s) => (settings = s));
    ipc.getNowPlaying().then((n) => (now = n));
    ipc.getOverlayLayout().then((l) => (editing = !!l?.edit));
    ipc.appInfo().then((i) => (version = i.version));
    return subscribe(
      on("settings", (s) => (settings = s)),
      on("track", (t) => (now = { ...now, track: t })),
      on("playback", (p) => (now = { ...now, status: p.status })),
      on("overlay-layout", (l) => (editing = !!l.edit)),
    );
  });
</script>

<div class="app">
  <aside class="side">
    <div class="brand">
      <span class="logo" aria-hidden="true">♪</span>
      <span>LyricHub</span>
    </div>
    <NowPlayingCard {now} />
    <nav>
      {#each pages as p (p.id)}
        <button class="nav" class:active={page === p.id} onclick={() => (page = p.id)}>{p.label}</button>
      {/each}
    </nav>
    <footer class="version">v{version}</footer>
  </aside>

  <main class="content">
    {#if error}
      <p class="error" role="alert">{error}</p>
    {/if}
    {#if settings}
      {#if page === "overlay"}
        <OverlayPage {settings} {update} {editing} />
      {:else if page === "appearance"}
        <AppearancePage {settings} {update} />
      {:else if page === "sync"}
        <SyncPage {settings} {update} {now} />
      {:else if page === "behavior"}
        <BehaviorPage {settings} {update} />
      {:else}
        <DiagnosticsPage {now} />
      {/if}
    {:else}
      <p class="muted">Lade Einstellungen …</p>
    {/if}
  </main>
</div>

<style>
  :global(:root) {
    --bg: #111318;
    --side: #0d0f13;
    --card: #1a1d24;
    --line: #2a2f3a;
    --text: #e8eaf0;
    --muted: #9aa1b2;
    --accent: #ffd36e;
    --accent-ink: #1a1400;
    --ok: #4ade80;
    --fail: #f87171;
    color-scheme: dark;
  }
  :global(body) {
    margin: 0;
    background: var(--bg);
    color: var(--text);
    font-family: "Inter Variable", "Segoe UI Variable", "Segoe UI", sans-serif;
    font-size: 14px;
  }
  :global(button) {
    background: #262b36;
    color: var(--text);
    border: 1px solid var(--line);
    border-radius: 8px;
    padding: 7px 12px;
    font: inherit;
    cursor: pointer;
  }
  :global(button:hover:not(:disabled)) {
    border-color: var(--muted);
  }
  :global(button.primary) {
    background: var(--accent);
    color: var(--accent-ink);
    border-color: transparent;
    font-weight: 600;
  }
  :global(button:disabled) {
    opacity: 0.6;
    cursor: default;
  }
  :global(.mono) {
    font-family: "Cascadia Mono", Consolas, monospace;
    font-size: 12px;
  }
  :global(.muted) {
    color: var(--muted);
  }

  .app {
    display: grid;
    grid-template-columns: 232px 1fr;
    height: 100vh;
  }
  .side {
    background: var(--side);
    border-right: 1px solid var(--line);
    padding: 18px 14px;
    display: flex;
    flex-direction: column;
    gap: 16px;
    min-height: 0;
  }
  .brand {
    display: flex;
    align-items: center;
    gap: 10px;
    font-size: 17px;
    font-weight: 650;
  }
  .logo {
    display: grid;
    place-items: center;
    width: 28px;
    height: 28px;
    border-radius: 8px;
    background: var(--accent);
    color: var(--accent-ink);
  }
  nav {
    display: grid;
    gap: 4px;
  }
  .nav {
    text-align: left;
    background: transparent;
    border-color: transparent;
    color: var(--muted);
  }
  .nav.active {
    background: #1c2029;
    border-color: var(--line);
    color: var(--text);
  }
  .version {
    margin-top: auto;
    font-size: 12px;
    color: var(--muted);
  }
  .content {
    overflow-y: auto;
    padding: 24px 28px 32px;
  }
  .error {
    color: var(--fail);
  }
</style>
