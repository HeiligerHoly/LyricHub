<script lang="ts">
  // Settings → Overlay (PLAN.md 9.7): edit mode, layout, monitor, anchor, fine position, width,
  // scale, romaji for Japanese lines (docs/romaji.md), pause fade-out. Theme, plate and font live
  // on the page Darstellung. Every change applies live to the overlay; sliders send at most one
  // patch per 60 ms. "Nur bei Vollbild-App zeigen" follows with the fullscreen detection
  // (Phase 5).
  import { onMount, untrack } from "svelte";
  import { num, t } from "#lib/shared/i18n.svelte.js";
  import { ipc } from "#lib/shared/ipc.js";
  import {
    LAYOUTS,
    SCALE_MAX,
    SCALE_MIN,
    WIDTH_PCT_MAX,
    WIDTH_PCT_MIN,
    type Monitor,
    type OverlaySettings,
    type Settings,
    type SettingsPatch,
  } from "#lib/shared/types.js";
  import AnchorPicker from "../AnchorPicker.svelte";

  interface Props {
    settings: Settings;
    update: (patch: SettingsPatch) => Promise<void>;
    editing: boolean;
  }
  let { settings, update, editing }: Props = $props();

  const PATCH_DELAY_MS = 60;
  /** Ignore echoes of our own patches this long after the last input. */
  const ECHO_GUARD_MS = 400;

  let monitors = $state<Monitor[]>([]);
  // Local copy, so a slider does not jump back when the echo of an older patch arrives.
  let o = $state<OverlaySettings>(untrack(() => ({ ...settings.overlay })));
  let lastInput = 0;
  $effect(() => {
    const next = settings.overlay;
    if (performance.now() - lastInput > ECHO_GUARD_MS) o = { ...next };
  });

  let pending: NonNullable<SettingsPatch["overlay"]> = {};
  let timer: ReturnType<typeof setTimeout> | undefined;
  function set<K extends keyof OverlaySettings>(key: K, value: OverlaySettings[K]) {
    o[key] = value;
    lastInput = performance.now();
    pending = { ...pending, [key]: value };
    clearTimeout(timer);
    timer = setTimeout(() => {
      const patch = pending;
      pending = {};
      update({ overlay: patch });
    }, PATCH_DELAY_MS);
  }

  const inputNum = (e: Event) => Number((e.currentTarget as HTMLInputElement).value);

  onMount(() => {
    ipc.listMonitors().then((m) => (monitors = m));
  });

  function monitorLabel(m: Monitor) {
    const label = t("overlayPage.monitor.item", { number: m.number, width: m.width, height: m.height, scale: Math.round(m.scale_factor * 100) });
    return m.primary ? `${label} · ${t("overlayPage.monitor.primary")}` : label;
  }
</script>

<!-- Text with <b>…</b> parts from the locales, drawn without {@html}. -->
{#snippet bold(text: string)}
  {#each text.split(/<\/?b>/) as part, i (i)}{#if i % 2}<b>{part}</b>{:else}{part}{/if}{/each}
{/snippet}

<h1>{t("nav.overlay")}</h1>

<section class="card">
  <div class="head">
    <div>
      <h2>{t("overlayPage.screen.title")}</h2>
      <p class="hint">{@render bold(t("overlayPage.screen.hint"))}</p>
    </div>
    <button class="primary" onclick={() => ipc.enterEditMode()} disabled={editing}>
      {editing ? t("overlayPage.screen.editing") : t("overlayPage.screen.startEdit")}
    </button>
  </div>
  <p class="hint">{@render bold(t("overlayPage.screen.shortcuts"))}</p>
</section>

<section class="card grid">
  <label class="label" for="layout">{t("overlayPage.layout")}</label>
  <div>
    <select
      id="layout"
      value={o.layout}
      onchange={(e) => {
        const value = (e.currentTarget as HTMLSelectElement).value as OverlaySettings["layout"];
        o.layout = value;
        update({ overlay: { layout: value } });
      }}
    >
      {#each LAYOUTS as l (l.id)}
        <option value={l.id}>{t(l.labelKey)}</option>
      {/each}
    </select>
    <span class="value layout-hint">{t("overlayPage.layout.hint")}</span>
  </div>

  <label class="label" for="monitor">{t("overlayPage.monitor")}</label>
  <div>
    <select
      id="monitor"
      value={o.monitor_id ?? ""}
      onchange={(e) => {
        const id = (e.currentTarget as HTMLSelectElement).value;
        o.monitor_id = id || null;
        update({ overlay: { monitor_id: id || null } });
      }}
    >
      <option value="">{t("overlayPage.monitor.auto")}</option>
      {#each monitors as m (m.id)}
        <option value={m.id}>{monitorLabel(m)}</option>
      {/each}
    </select>
  </div>

  <span class="label">{t("overlayPage.anchor")}</span>
  <div class="anchor">
    <AnchorPicker value={o.anchor} onpick={(a) => ipc.setOverlayAnchor(a)} />
  </div>

  <label class="label" for="ox">{t("overlayPage.fine")}</label>
  <div class="pair">
    <span class="axis">X</span>
    <input
      id="ox"
      type="range"
      min="-50"
      max="50"
      step="0.1"
      value={o.offset_x_pct}
      oninput={(e) => set("offset_x_pct", inputNum(e))}
    />
    <input class="number" type="number" step="0.1" value={o.offset_x_pct} onchange={(e) => set("offset_x_pct", inputNum(e))} />
    <span class="unit">%</span>
    <span class="axis">Y</span>
    <input
      type="range"
      min="-50"
      max="50"
      step="0.1"
      value={o.offset_y_pct}
      oninput={(e) => set("offset_y_pct", inputNum(e))}
      aria-label={t("overlayPage.fine.y")}
    />
    <input class="number" type="number" step="0.1" value={o.offset_y_pct} onchange={(e) => set("offset_y_pct", inputNum(e))} aria-label={t("overlayPage.fine.yPct")} />
    <span class="unit">%</span>
  </div>
  <p class="hint wide">{t("overlayPage.fine.hint")}</p>

  <label class="label" for="width">{t("overlayPage.width")}</label>
  <div class="slider">
    <input
      id="width"
      type="range"
      min={WIDTH_PCT_MIN}
      max={WIDTH_PCT_MAX}
      step="1"
      value={o.width_pct}
      oninput={(e) => set("width_pct", inputNum(e))}
    />
    <span class="value">{t("overlayPage.width.value", { pct: Math.round(o.width_pct) })}</span>
  </div>

  <label class="label" for="scale">{t("overlayPage.scale")}</label>
  <div class="slider">
    <input id="scale" type="range" min={SCALE_MIN} max={SCALE_MAX} step="0.05" value={o.scale} oninput={(e) => set("scale", inputNum(e))} />
    <span class="value">{num(o.scale, 2)}×</span>
  </div>
  <p class="hint wide">{t("overlayPage.scale.hint")}</p>

  <label class="label" for="romaji">{t("overlayPage.romaji")}</label>
  <div class="slider">
    <input
      id="romaji"
      class="check"
      type="checkbox"
      checked={settings.appearance.romaji}
      onchange={(e) => update({ appearance: { romaji: (e.currentTarget as HTMLInputElement).checked } })}
    />
    <span class="value">{t("overlayPage.romaji.value")}</span>
  </div>

  <label class="label" for="pause">{t("overlayPage.pause")}</label>
  <div class="slider">
    <input
      id="pause"
      class="number"
      type="number"
      min="0"
      max="3600"
      step="1"
      value={o.hide_after_pause_s}
      onchange={(e) => set("hide_after_pause_s", Math.max(0, Math.round(inputNum(e))))}
    />
    <span class="value">{t("overlayPage.pause.value")}</span>
  </div>

  <label class="label" for="fullscreen">{t("overlayPage.onlyFullscreen")}</label>
  <div class="slider">
    <input
      id="fullscreen"
      class="check"
      type="checkbox"
      checked={o.only_fullscreen}
      onchange={(e) => set("only_fullscreen", (e.currentTarget as HTMLInputElement).checked)}
    />
    <span class="value">{t("overlayPage.onlyFullscreen.value")}</span>
  </div>
</section>

<section class="card actions">
  <button onclick={() => ipc.toggleOverlay()}>{t("overlayPage.toggle")}</button>
  <button onclick={() => ipc.resetOverlayGeometry()}>{t("overlayPage.reset")}</button>
</section>

<style>
  h1 {
    margin: 0 0 18px;
    font-size: 22px;
  }
  h2 {
    margin: 0 0 4px;
    font-size: 15px;
  }
  .card {
    background: var(--card);
    border: 1px solid var(--line);
    border-radius: 12px;
    padding: 16px 18px;
    margin-bottom: 14px;
  }
  .head {
    display: flex;
    justify-content: space-between;
    align-items: flex-start;
    gap: 16px;
  }
  .hint {
    margin: 6px 0 0;
    color: var(--muted);
    font-size: 12.5px;
  }
  .grid {
    display: grid;
    grid-template-columns: 190px 1fr;
    gap: 14px 16px;
    align-items: center;
  }
  .grid .hint.wide {
    grid-column: 2;
    margin-top: -8px;
  }
  .label {
    color: var(--muted);
  }
  .anchor {
    display: flex;
    align-items: center;
  }
  .pair,
  .slider {
    display: flex;
    align-items: center;
    gap: 10px;
    flex-wrap: wrap;
  }
  .axis {
    color: var(--muted);
    width: 12px;
  }
  .pair input[type="range"] {
    width: 130px;
  }
  .slider input[type="range"] {
    width: 260px;
  }
  .value,
  .unit {
    color: var(--muted);
  }
  input[type="range"],
  .check {
    accent-color: var(--accent);
  }
  .check {
    margin: 0;
  }
  .number,
  select {
    background: #12151b;
    color: var(--text);
    border: 1px solid var(--line);
    border-radius: 7px;
    padding: 6px 8px;
    font: inherit;
  }
  .number {
    width: 72px;
  }
  select {
    min-width: 320px;
  }
  .layout-hint {
    margin-left: 12px;
  }
  .actions {
    display: flex;
    gap: 8px;
  }
</style>
