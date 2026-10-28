<script lang="ts">
  // Settings → Verhalten (PLAN.md 9.7): the player to follow (13.4, experimental for browsers),
  // autostart, start minimised, interface language, logs.
  // An update check follows only with a release.
  import { onMount } from "svelte";
  import { t } from "#lib/shared/i18n.svelte.js";
  import { ipc } from "#lib/shared/ipc.js";
  import { DEFAULT_PLAYER, type AppInfo, type Language, type PlayerSession, type Settings, type SettingsPatch } from "#lib/shared/types.js";

  let { settings, update }: { settings: Settings; update: (patch: SettingsPatch) => Promise<void> } = $props();

  let info = $state<AppInfo | null>(null);
  const refreshInfo = () => ipc.appInfo().then((i) => (info = i));
  onMount(refreshInfo);

  async function setAutostart(enabled: boolean) {
    await update({ behavior: { autostart: enabled } });
    refreshInfo();
  }

  // ---- Player ----
  let sessions = $state<PlayerSession[]>([]);
  const refreshPlayers = () => ipc.listPlayers().then((s) => (sessions = s));
  onMount(refreshPlayers);
  /** Other players than Spotify; a browser is recognised by what it plays. */
  const others = $derived(sessions.filter((s) => s.kind !== "spotify"));
  const savedMissing = $derived(
    settings.player.app_id !== DEFAULT_PLAYER && !others.some((s) => s.app_id === settings.player.app_id),
  );
  /** Long titles would make the select as wide as the longest option. */
  const TITLE_MAX = 40;
  function playerLabel(s: PlayerSession): string {
    const name = t(`behavior.player.${s.kind}`);
    const title = s.title.length > TITLE_MAX ? `${s.title.slice(0, TITLE_MAX - 1).trimEnd()}…` : s.title;
    const what = title ? ` – ${title}` : "";
    return `${name}${what}${s.has_timeline ? "" : ` (${t("behavior.player.noTimeline")})`}`;
  }

  // The variable name stays out of the locales; the text around it is translated.
  const [debugBefore, debugAfter] = $derived(t("behavior.files.debug").split("{var}"));
</script>

<h1>{t("nav.behavior")}</h1>

<section class="card grid">
  <label class="label" for="player">{t("behavior.player")}</label>
  <div class="player">
    <select
      id="player"
      value={settings.player.app_id}
      onchange={(e) => update({ player: { app_id: (e.currentTarget as HTMLSelectElement).value } })}
    >
      <option value={DEFAULT_PLAYER}>{t("behavior.player.spotify")}</option>
      {#each others as s (s.app_id)}
        <option value={s.app_id}>{playerLabel(s)}</option>
      {/each}
      {#if savedMissing}<option value={settings.player.app_id}>{t("behavior.player.saved")}</option>{/if}
    </select>
    <button onclick={refreshPlayers}>{t("behavior.player.refresh")}</button>
    {#if others.length === 0}<p class="hint">{t("behavior.player.none")}</p>{/if}
    <p class="hint">{t("behavior.player.hint")}</p>
  </div>
</section>

<section class="card">
  <label class="toggle">
    <input
      type="checkbox"
      checked={settings.behavior.autostart}
      onchange={(e) => setAutostart((e.currentTarget as HTMLInputElement).checked)}
    />
    <span>
      <b>{t("behavior.autostart")}</b>
      <span class="hint">{t("behavior.autostart.hint")}</span>
      {#if info && info.autostart_registered !== null && info.autostart_registered !== settings.behavior.autostart}
        <span class="warn">{t("behavior.autostart.mismatch")}</span>
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
      <b>{t("behavior.minimized")}</b>
      <span class="hint">{t("behavior.minimized.hint")}</span>
    </span>
  </label>
</section>

<section class="card grid">
  <label class="label" for="language">{t("behavior.language")}</label>
  <div>
    <select
      id="language"
      value={settings.behavior.language}
      onchange={(e) => update({ behavior: { language: (e.currentTarget as HTMLSelectElement).value as Language } })}
    >
      <option value="system">{t("behavior.language.system")}</option>
      <!-- Language names stay in their own language, so each is found in either interface. -->
      <option value="de">Deutsch</option>
      <option value="en">English</option>
    </select>
  </div>
</section>

<section class="card">
  <h2>{t("behavior.files")}</h2>
  <dl class="kv">
    <dt>{t("behavior.files.settings")}</dt>
    <dd class="mono">{info?.settings_path ?? "…"}</dd>
    <dt>{t("behavior.files.logs")}</dt>
    <dd class="mono">{info?.log_dir ?? "…"}</dd>
    <dt>{t("behavior.files.version")}</dt>
    <dd>{info?.version ?? "…"}</dd>
  </dl>
  <div class="actions">
    <button onclick={() => ipc.openLogsFolder()}>{t("common.openLogs")}</button>
  </div>
  <p class="hint">{debugBefore}<span class="mono">LYRICHUB_LOG=debug</span>{debugAfter}</p>
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
  /* Label column and select as on the pages Overlay and Darstellung. The second column may
     shrink below its content, so a long select cannot push the card wider than the window. */
  .grid {
    grid-template-columns: minmax(110px, 190px) minmax(0, 1fr);
    gap: 14px 16px;
    align-items: center;
  }
  .label {
    color: var(--muted);
  }
  .player {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
  }
  .player select {
    flex: 1 1 260px;
    min-width: 0;
  }
  .player .hint {
    flex-basis: 100%;
  }
  /* A narrow settings window leaves too little room beside the label: stack them. */
  @media (max-width: 760px) {
    .grid {
      grid-template-columns: minmax(0, 1fr);
      gap: 6px;
    }
  }
  select {
    min-width: min(320px, 100%);
    max-width: 100%;
    background: #12151b;
    color: var(--text);
    border: 1px solid var(--line);
    border-radius: 7px;
    padding: 6px 8px;
    font: inherit;
  }
</style>
