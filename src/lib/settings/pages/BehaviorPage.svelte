<script lang="ts">
  // Settings → Verhalten (PLAN.md 9.7): autostart, start minimised, logs. The language choice
  // follows with i18n in Phase 4, an update check only with a release.
  import { onMount } from "svelte";
  import { ipc } from "$lib/shared/ipc";
  import type { AppInfo, Settings, SettingsPatch } from "$lib/shared/types";

  let { settings, update }: { settings: Settings; update: (patch: SettingsPatch) => Promise<void> } = $props();

  let info = $state<AppInfo | null>(null);
  const refreshInfo = () => ipc.appInfo().then((i) => (info = i));
  onMount(refreshInfo);

  async function setAutostart(enabled: boolean) {
    await update({ behavior: { autostart: enabled } });
    refreshInfo();
  }
</script>

<h1>Verhalten</h1>

<section class="card">
  <label class="toggle">
    <input
      type="checkbox"
      checked={settings.behavior.autostart}
      onchange={(e) => setAutostart((e.currentTarget as HTMLInputElement).checked)}
    />
    <span>
      <b>Mit Windows starten</b>
      <span class="hint">LyricHub startet still im Tray und öffnet kein Fenster.</span>
      {#if info && info.autostart_registered !== null && info.autostart_registered !== settings.behavior.autostart}
        <span class="warn">Der Windows-Eintrag weicht ab und wird beim nächsten Start korrigiert.</span>
      {/if}
    </span>
  </label>
  <label class="toggle">
    <input
      type="checkbox"
      checked={settings.behavior.start_minimized}
      onchange={(e) => update({ behavior: { start_minimized: (e.currentTarget as HTMLInputElement).checked } })}
    />
    <span>
      <b>Auch beim manuellen Start nur im Tray starten</b>
      <span class="hint">Sonst öffnet sich dieses Fenster, wenn LyricHub von Hand gestartet wird.</span>
    </span>
  </label>
</section>

<section class="card">
  <h2>Sprache</h2>
  <p class="hint">Deutsch. Englisch folgt mit der Übersetzung der Oberfläche.</p>
</section>

<section class="card">
  <h2>Dateien</h2>
  <dl class="kv">
    <dt>Einstellungen</dt>
    <dd class="mono">{info?.settings_path ?? "…"}</dd>
    <dt>Logs</dt>
    <dd class="mono">{info?.log_dir ?? "…"}</dd>
    <dt>Version</dt>
    <dd>{info?.version ?? "…"}</dd>
  </dl>
  <div class="actions">
    <button onclick={() => ipc.openLogsFolder()}>Logs öffnen</button>
  </div>
  <p class="hint">Mehr Details im Log mit der Umgebungsvariable <span class="mono">LYRICHUB_LOG=debug</span>.</p>
</section>

<style>
  h1 {
    margin: 0 0 18px;
    font-size: 22px;
  }
  h2 {
    margin: 0 0 8px;
    font-size: 15px;
  }
  .card {
    background: var(--card);
    border: 1px solid var(--line);
    border-radius: 12px;
    padding: 16px 18px;
    margin-bottom: 14px;
    display: grid;
    gap: 14px;
  }
  .toggle {
    display: flex;
    gap: 12px;
    align-items: flex-start;
    cursor: pointer;
  }
  .toggle input {
    margin-top: 3px;
    accent-color: var(--accent);
  }
  .toggle > span {
    display: grid;
    gap: 2px;
  }
  .hint {
    margin: 0;
    color: var(--muted);
    font-size: 12.5px;
  }
  .warn {
    color: var(--accent);
    font-size: 12.5px;
  }
  .kv {
    display: grid;
    grid-template-columns: 120px 1fr;
    gap: 6px 12px;
    margin: 0;
  }
  .kv dt {
    color: var(--muted);
  }
  .kv dd {
    margin: 0;
    word-break: break-all;
  }
  .actions {
    display: flex;
    gap: 8px;
  }
</style>
