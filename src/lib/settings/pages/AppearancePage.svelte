<script lang="ts">
  // Settings → Darstellung (PLAN.md 9.5, 9.7): theme, accent (from the cover or fixed), plate
  // opacity, font, reduced motion and the custom theme. While this page is open the overlay
  // shows sample lines in the current look (live preview); every change applies at once.
  // Sliders and colour pickers send at most one patch per 60 ms.
  import { onMount, untrack } from "svelte";
  import { ipc } from "$lib/shared/ipc";
  import { RADIUS_MAX_PX, type AppearanceSettings, type FontList, type Settings, type SettingsPatch, type Theme } from "$lib/shared/types";

  interface Props {
    settings: Settings;
    update: (patch: SettingsPatch) => Promise<void>;
  }
  let { settings, update }: Props = $props();

  const PATCH_DELAY_MS = 60;
  const ECHO_GUARD_MS = 400;

  const THEMES: { id: Theme; label: string; note: string; plate: string; text: string; base: string; accent: string; outline?: boolean }[] = [
    { id: "glass_dark", label: "Glass Dark", note: "Standard, dunkle Platte", plate: "rgba(12,12,16,.75)", text: "#fff", base: "rgba(255,255,255,.55)", accent: "#ffd36e" },
    { id: "glass_light", label: "Glass Light", note: "helle Platte, dunkle Schrift", plate: "rgba(255,255,255,.8)", text: "#111", base: "rgba(17,17,17,.5)", accent: "#9a6700" },
    { id: "neon", label: "Neon", note: "ohne Platte, Kontur und Leuchten", plate: "transparent", text: "#fff", base: "rgba(255,255,255,.72)", accent: "#ffd36e", outline: true },
    { id: "mono", label: "Mono", note: "Schwarz/Weiß, hoher Kontrast", plate: "rgba(0,0,0,.9)", text: "#fff", base: "rgba(255,255,255,.55)", accent: "#fff" },
    { id: "custom", label: "Eigenes", note: "Farben, Kontur und Radius frei", plate: "rgba(12,12,16,.75)", text: "#fff", base: "rgba(255,255,255,.55)", accent: "#ffd36e" },
  ];

  let fonts = $state<FontList | null>(null);

  // Local copy, so a slider or colour picker does not jump back when an older echo arrives.
  let a = $state<AppearanceSettings>(untrack(() => structuredClone($state.snapshot(settings.appearance))));
  let lastInput = 0;
  $effect(() => {
    const next = settings.appearance;
    if (performance.now() - lastInput > ECHO_GUARD_MS) a = structuredClone($state.snapshot(next));
  });

  let pending: NonNullable<SettingsPatch["appearance"]> = {};
  let timer: ReturnType<typeof setTimeout> | undefined;
  /** Debounced patch for continuous inputs; `now` sends at once (selects, checkboxes). */
  function send(patch: NonNullable<SettingsPatch["appearance"]>, now = false) {
    lastInput = performance.now();
    pending = { ...pending, ...patch, custom: patch.custom ? { ...pending.custom, ...patch.custom } : pending.custom };
    if (!pending.custom) delete pending.custom;
    clearTimeout(timer);
    const flush = () => {
      const p = pending;
      pending = {};
      update({ appearance: p });
    };
    if (now) flush();
    else timer = setTimeout(flush, PATCH_DELAY_MS);
  }
  function setCustom<K extends keyof AppearanceSettings["custom"]>(key: K, value: AppearanceSettings["custom"][K], now = false) {
    a.custom[key] = value;
    send({ custom: { [key]: value } }, now);
  }

  const val = (e: Event) => (e.currentTarget as HTMLInputElement).value;
  const checked = (e: Event) => (e.currentTarget as HTMLInputElement).checked;
  const hasPlate = $derived(a.theme === "glass_dark" || a.theme === "glass_light" || a.theme === "mono" || (a.theme === "custom" && a.custom.plate));

  onMount(() => {
    ipc.setOverlayPreview(true);
    ipc.listFonts().then((f) => (fonts = f));
    return () => {
      clearTimeout(timer);
      ipc.setOverlayPreview(false);
    };
  });
</script>

<h1>Darstellung</h1>

<section class="card">
  <h2>Theme</h2>
  <p class="hint">Solange diese Seite offen ist, zeigt das Overlay eine Beispielzeile im gewählten Look.</p>
  <div class="themes" role="radiogroup" aria-label="Theme">
    {#each THEMES as t (t.id)}
      <button
        class="theme"
        class:active={a.theme === t.id}
        role="radio"
        aria-checked={a.theme === t.id}
        onclick={() => {
          a.theme = t.id;
          send({ theme: t.id }, true);
        }}
      >
        <span class="swatch" class:outline={t.outline} style="--p: {t.plate}; --t: {t.text}; --b: {t.base}; --a: {t.accent}">
          <span class="lit">Kara</span><span class="rest">oke</span>
        </span>
        <span class="name">{t.label}</span>
        <span class="note">{t.note}</span>
      </button>
    {/each}
  </div>
</section>

<section class="card grid">
  <span class="label">Akzentfarbe</span>
  <div class="row">
    <label class="radio">
      <input type="radio" name="accent" checked={a.accent === "cover"} onchange={() => ((a.accent = "cover"), send({ accent: "cover" }, true))} />
      aus dem Cover
    </label>
    <label class="radio">
      <input type="radio" name="accent" checked={a.accent === "fixed"} onchange={() => ((a.accent = "fixed"), send({ accent: "fixed" }, true))} />
      fest
    </label>
    <input
      class="color"
      type="color"
      value={a.accent_color}
      aria-label="Feste Akzentfarbe"
      oninput={(e) => {
        a.accent_color = val(e);
        send({ accent_color: val(e) });
      }}
    />
  </div>
  <p class="hint wide">
    Aus dem Cover nimmt LyricHub die kräftigste Farbe des Albumcovers und hellt sie so weit auf, dass sie sich gut von der Platte abhebt.
    {#if a.theme === "mono"}Im Theme Mono bleibt die Füllung weiß.{/if}
  </p>

  <label class="label" for="opacity">Deckkraft der Platte</label>
  <div class="row">
    <input
      id="opacity"
      type="range"
      min="0"
      max="100"
      step="5"
      disabled={!hasPlate}
      value={Math.round(a.plate_opacity * 100)}
      oninput={(e) => {
        a.plate_opacity = Number(val(e)) / 100;
        send({ plate_opacity: a.plate_opacity });
      }}
    />
    <span class="value">{hasPlate ? `${Math.round(a.plate_opacity * 100)} % · höher liest sich über hellen Spielszenen besser` : "dieses Theme hat keine Platte"}</span>
  </div>

  <label class="label" for="font">Schrift</label>
  <div class="row">
    <select
      id="font"
      value={a.font}
      onchange={(e) => {
        a.font = (e.currentTarget as HTMLSelectElement).value;
        send({ font: a.font }, true);
      }}
    >
      <option value="">Inter (mitgeliefert)</option>
      {#if a.font && fonts && !fonts.families.includes(a.font)}<option value={a.font}>{a.font} (nicht installiert)</option>{/if}
      {#each fonts?.families ?? [] as f (f)}
        <option value={f}>{f}</option>
      {/each}
    </select>
  </div>
  {#if fonts && !fonts.japanese_ui_font}
    <p class="hint wide warn">
      Für japanische Zeilen fehlt eine passende Schrift (Yu Gothic UI oder Meiryo UI). Windows-Einstellungen → System → Optionale Features →
      „Japanische ergänzende Schriftarten“ hinzufügen.
    </p>
  {/if}

  <label class="label" for="motion">Bewegung reduzieren</label>
  <div class="row">
    <input id="motion" class="check" type="checkbox" checked={a.reduced_motion} onchange={(e) => ((a.reduced_motion = checked(e)), send({ reduced_motion: checked(e) }, true))} />
    <span class="value">Übergänge höchstens 80 ms, kein Leuchten einzelner Wörter. Gilt auch, wenn Windows Animationen ausschaltet.</span>
  </div>
</section>

{#if a.theme === "custom"}
  <section class="card grid">
    <h2 class="wide-title">Eigenes Theme</h2>

    <label class="label" for="plate">Platte</label>
    <div class="row">
      <input id="plate" class="check" type="checkbox" checked={a.custom.plate} onchange={(e) => setCustom("plate", checked(e), true)} />
      <input class="color" type="color" value={a.custom.plate_color} disabled={!a.custom.plate} aria-label="Farbe der Platte" oninput={(e) => setCustom("plate_color", val(e))} />
      <span class="value">Farbe; die Deckkraft steht oben</span>
    </div>

    <label class="label" for="text">Textfarbe</label>
    <div class="row">
      <input id="text" class="color" type="color" value={a.custom.text_color} oninput={(e) => setCustom("text_color", val(e))} />
      <span class="value">Die Füllung nutzt die Akzentfarbe.</span>
    </div>

    <label class="label" for="outline">Kontur</label>
    <div class="row">
      <input id="outline" class="check" type="checkbox" checked={a.custom.outline} onchange={(e) => setCustom("outline", checked(e), true)} />
      <span class="value">dunkler Rand und Schatten um die Schrift, gut ohne Platte</span>
    </div>

    <label class="label" for="radius">Eckenradius</label>
    <div class="row">
      <input id="radius" type="range" min="0" max={RADIUS_MAX_PX} step="1" value={a.custom.radius_px} oninput={(e) => setCustom("radius_px", Number(val(e)))} />
      <span class="value">{a.custom.radius_px} px</span>
    </div>
  </section>
{/if}

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
  .hint {
    margin: 6px 0 0;
    color: var(--muted);
    font-size: 12.5px;
  }
  .warn {
    color: #f0b35a;
  }
  .themes {
    display: grid;
    grid-template-columns: repeat(5, minmax(0, 1fr));
    gap: 10px;
    margin-top: 12px;
  }
  .theme {
    display: grid;
    gap: 4px;
    justify-items: start;
    padding: 10px;
    text-align: left;
    background: #12151b;
    border: 1px solid var(--line);
    border-radius: 10px;
    color: var(--text);
    font: inherit;
    cursor: pointer;
  }
  .theme.active {
    border-color: var(--accent);
    box-shadow: 0 0 0 1px var(--accent);
  }
  /* A tiny overlay: checkered ground, so plate-less themes show how they sit on a game. */
  .swatch {
    display: block;
    width: 100%;
    box-sizing: border-box;
    padding: 8px 10px;
    border-radius: 8px;
    font-weight: 700;
    font-size: 17px;
    background:
      linear-gradient(var(--p), var(--p)),
      repeating-conic-gradient(#3a4150 0 25%, #596273 0 50%) 0 0 / 14px 14px;
  }
  .swatch.outline {
    -webkit-text-stroke: 2px rgba(0, 0, 0, 0.85);
    paint-order: stroke fill;
    text-shadow: 0 2px 6px rgba(0, 0, 0, 0.7);
  }
  .swatch .lit {
    color: var(--a);
  }
  .swatch .rest {
    color: var(--b);
  }
  .name {
    font-weight: 600;
  }
  .note {
    color: var(--muted);
    font-size: 12px;
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
  .wide-title {
    grid-column: 1 / -1;
  }
  .label {
    color: var(--muted);
  }
  .row {
    display: flex;
    align-items: center;
    gap: 10px;
    flex-wrap: wrap;
  }
  .radio {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .value {
    color: var(--muted);
  }
  input[type="range"] {
    width: 260px;
  }
  input[type="range"],
  .check,
  input[type="radio"] {
    accent-color: var(--accent);
  }
  .check {
    margin: 0;
  }
  .color {
    width: 40px;
    height: 28px;
    padding: 0 2px;
    background: #12151b;
    border: 1px solid var(--line);
    border-radius: 7px;
  }
  .color:disabled,
  input:disabled {
    opacity: 0.45;
  }
  select {
    min-width: 320px;
    background: #12151b;
    color: var(--text);
    border: 1px solid var(--line);
    border-radius: 7px;
    padding: 6px 8px;
    font: inherit;
  }
</style>
