<script lang="ts">
  // Settings → Plugins (PLAN-plugins.md 4.3, 4.7): installed plugins with their state, a switch
  // each, and installing from a ZIP. The ZIP is checked first; the user confirms name, version,
  // permissions and SHA-256 before anything is written. A new plugin is off; switching one on
  // the first time asks once more, because plugins are ordinary programs.
  import { onMount } from "svelte";
  import { t, type Key } from "#lib/shared/i18n.svelte.js";
  import { ipc, on, subscribe } from "#lib/shared/ipc.js";
  import type { PluginInfo, Settings, ZipInfo } from "#lib/shared/types.js";

  let { settings }: { settings: Settings } = $props();

  let plugins = $state<PluginInfo[]>([]);
  let staged = $state<ZipInfo | null>(null);
  let error = $state<string | null>(null);
  let busy = $state(false);
  let picker: HTMLInputElement;

  const lang = $derived(settings.behavior.language === "en" ? "en" : "de");

  onMount(() => {
    ipc.listPlugins().then((p) => (plugins = p));
    return subscribe(on("plugins", (p) => (plugins = p)));
  });

  async function run<T>(action: () => Promise<T>): Promise<T | undefined> {
    busy = true;
    error = null;
    try {
      return await action();
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }

  async function picked(e: Event) {
    const file = (e.currentTarget as HTMLInputElement).files?.[0];
    picker.value = "";
    if (!file) return;
    const info = await run(async () => ipc.checkPluginZip(new Uint8Array(await file.arrayBuffer())));
    if (info) staged = info;
  }

  async function install() {
    const list = await run(() => ipc.installStagedPlugin());
    if (list) {
      plugins = list;
      staged = null;
    }
  }

  function cancel() {
    staged = null;
    ipc.discardStagedPlugin();
  }

  async function toggle(p: PluginInfo, enabled: boolean) {
    if (enabled && !confirm(t("plugins.enableWarning", { name: p.name }))) {
      plugins = [...plugins];
      return;
    }
    const list = await run(() => ipc.setPluginEnabled(p.id, enabled));
    if (list) plugins = list;
  }

  async function setSetting(p: PluginInfo, key: string, value: boolean) {
    const list = await run(() => ipc.setPluginSetting(p.id, key, value));
    if (list) plugins = list;
  }

  async function remove(p: PluginInfo) {
    if (!confirm(t("plugins.removeConfirm", { name: p.name }))) return;
    const list = await run(() => ipc.removePlugin(p.id));
    if (list) plugins = list;
  }

  function stateText(p: PluginInfo): string {
    if (p.state === "restarting" && p.detail) {
      const [attempt, seconds] = p.detail.split("/");
      return t("plugins.state.restarting", { attempt, seconds });
    }
    return t(`plugins.state.${p.state}` as Key);
  }

  const permissionText = (perm: string) =>
    perm === "audio:player"
      ? t("plugins.permission.audio")
      : perm.startsWith("network:")
        ? t("plugins.permission.network", { host: perm.slice("network:".length) })
        : perm;

  const describe = (d: Record<string, string>) => d[lang] ?? d.en ?? Object.values(d)[0] ?? "";
  const megabytes = (bytes: number) => (bytes / 1024 / 1024).toFixed(1);
</script>

<h1>{t("nav.plugins")}</h1>

<section class="card">
  <p class="hint">{t("plugins.intro")}</p>
  <div class="actions">
    <button onclick={() => picker.click()} disabled={busy}>{t("plugins.install")}</button>
    <input bind:this={picker} type="file" accept=".zip" hidden onchange={picked} />
  </div>
  {#if error}<p class="warn">{error}</p>{/if}
</section>

{#if staged}
  <section class="card confirm">
    <h2>{t("plugins.confirm.title", { name: staged.name, version: staged.version })}</h2>
    {#if describe(staged.description)}<p>{describe(staged.description)}</p>{/if}
    {#if staged.installed_version}<p class="hint">{t("plugins.confirm.replaces", { version: staged.installed_version })}</p>{/if}
    {#if !staged.compatible}<p class="warn">{t("plugins.confirm.incompatible")}</p>{/if}
    <p class="label">{t("plugins.permissions")}</p>
    {#if staged.permissions.length}
      <ul>{#each staged.permissions as perm (perm)}<li>{permissionText(perm)}</li>{/each}</ul>
    {:else}
      <p class="hint">{t("plugins.permissions.none")}</p>
    {/if}
    <p class="hint">{t("plugins.confirm.size", { mb: megabytes(staged.bytes) })}</p>
    <p class="hint mono">SHA-256 {staged.sha256}</p>
    <p class="warn">{t("plugins.trust")}</p>
    <div class="actions">
      <button class="primary" onclick={install} disabled={busy}>{t("plugins.confirm.install")}</button>
      <button onclick={cancel} disabled={busy}>{t("plugins.confirm.cancel")}</button>
    </div>
  </section>
{/if}

{#if plugins.length === 0}
  <section class="card"><p class="hint">{t("plugins.none")}</p></section>
{/if}

{#each plugins as p (p.id)}
  <section class="card plugin">
    <div class="head">
      <label class="toggle">
        <input
          type="checkbox"
          checked={p.enabled}
          disabled={busy || p.state === "invalid" || p.state === "incompatible"}
          onchange={(e) => toggle(p, (e.currentTarget as HTMLInputElement).checked)}
        />
        <span>
          <b>{p.name}</b>
          {#if p.version}<span class="hint">{p.version}</span>{/if}
        </span>
      </label>
      <span class="state" class:ok={p.state === "running"} class:bad={p.state === "failed" || p.state === "invalid" || p.state === "incompatible"}>
        {stateText(p)}
      </span>
    </div>
    {#if describe(p.description)}<p class="hint">{describe(p.description)}</p>{/if}
    {#if p.status_text}
      <p class="status">
        {p.status_text}{#if p.progress != null}<span class="hint"> · {Math.round(p.progress * 100)} %</span>{/if}
      </p>
    {/if}
    {#if p.detail && p.state !== "restarting"}<p class="warn">{p.detail}</p>{/if}
    {#each p.settings as s (s.id)}
      <label class="setting">
        <input type="checkbox" checked={s.value} disabled={busy} onchange={(e) => setSetting(p, s.id, (e.currentTarget as HTMLInputElement).checked)} />
        <span>
          {describe(s.label)}
          {#if describe(s.help)}<span class="hint block">{describe(s.help)}</span>{/if}
        </span>
      </label>
    {/each}
    {#if p.permissions.length}
      <p class="hint">{t("plugins.permissions")}: {p.permissions.map(permissionText).join(" · ")}</p>
    {/if}
    <div class="actions">
      <button onclick={() => remove(p)} disabled={busy}>{t("plugins.remove")}</button>
    </div>
  </section>
{/each}

<style>
  /* Page frame as on the other settings pages. */
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
    gap: 10px;
  }
  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }
  .hint {
    margin: 0;
    color: var(--muted);
    font-size: 12.5px;
  }
  .warn {
    margin: 0;
    color: var(--accent);
    font-size: 12.5px;
  }
  .label {
    margin: 0;
    color: var(--muted);
  }
  .confirm ul {
    margin: 0;
    padding-left: 20px;
  }
  .confirm .mono {
    word-break: break-all;
  }
  .plugin .head {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
  }
  .toggle {
    display: flex;
    align-items: center;
    gap: 10px;
    cursor: pointer;
  }
  .toggle input {
    accent-color: var(--accent);
  }
  .toggle .hint {
    margin-left: 6px;
  }
  .state {
    font-size: 12.5px;
    color: var(--muted);
  }
  .state.ok {
    color: #7bd88f;
  }
  .state.bad {
    color: var(--accent);
  }
  .status {
    margin: 0;
  }
  .setting {
    display: flex;
    align-items: flex-start;
    gap: 10px;
    cursor: pointer;
  }
  .setting input {
    accent-color: var(--accent);
    margin-top: 3px;
  }
  .hint.block {
    display: block;
  }
</style>
