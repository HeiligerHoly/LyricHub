<script lang="ts">
  // Sync Lab (PLAN.md 5.9): the last 60 s of position reports as residuals against the model
  // (one axis, ms), what the clock did with each, taps of a sync measurement, plus live
  // values, a table view and the diagnostics export. Polls four times a second while the
  // page is visible; nothing runs when the settings window is closed.
  import { onMount } from "svelte";
  import { t } from "#lib/shared/i18n.svelte.js";
  import { formatMs, ipc } from "#lib/shared/ipc.js";
  import type { LabEntry, SyncLab } from "#lib/shared/types.js";

  const W = 720;
  const H = 200;
  const PAD = { l: 44, r: 12, t: 12, b: 24 };
  const RANGE_MS = 300;
  const WINDOW_S = 60;

  // Categorical slots 1–5 of the dataviz reference palette, dark steps (validated on #1a1a19).
  // Labels are keys; t() runs in the template and in `marks`, so they follow the language.
  const CATEGORIES = [
    { id: "none", labelKey: "lab.cat.none", color: "#3987e5", shape: "circle" },
    { id: "slew", labelKey: "lab.cat.slew", color: "#d95926", shape: "circle" },
    { id: "jump", labelKey: "lab.cat.jump", color: "#199e70", shape: "square" },
    { id: "seek", labelKey: "lab.cat.seek", color: "#c98500", shape: "triangle" },
    { id: "tap", labelKey: "lab.cat.tap", color: "#d55181", shape: "diamond" },
  ] as const;
  type CategoryId = (typeof CATEGORIES)[number]["id"];

  let lab = $state<SyncLab | null>(null);
  let exportPath = $state<string | null>(null);
  let showTable = $state(false);

  const x = (t: number, now: number) => PAD.l + (1 - (now - t) / (WINDOW_S * 1000)) * (W - PAD.l - PAD.r);
  const y = (ms: number) => {
    const clamped = Math.max(-RANGE_MS, Math.min(RANGE_MS, ms));
    return PAD.t + (1 - (clamped + RANGE_MS) / (2 * RANGE_MS)) * (H - PAD.t - PAD.b);
  };

  function category(e: LabEntry): CategoryId | null {
    if (e.kind === "tap") return "tap";
    if (e.kind !== "obs") return null;
    if (e.update === "slew") return "slew";
    if (e.update === "jump") return "jump";
    if (e.update === "seek" || e.update === "adopt") return "seek";
    return "none";
  }

  const marks = $derived.by(() => {
    if (!lab) return [];
    const now = lab.now_ms;
    return lab.entries
      .map((e) => ({ e, c: category(e) }))
      .filter((m) => m.c !== null)
      .map(({ e, c }) => {
        const value = e.kind === "tap" ? e.value_ms : (e.residual_ms ?? (c === "seek" ? RANGE_MS : 0));
        const cat = CATEGORIES.find((k) => k.id === c)!;
        const tip =
          e.kind === "tap"
            ? t(value > 0 ? "lab.tip.tapEarly" : value < 0 ? "lab.tip.tapLate" : "lab.tip.tapExact", { ms: Math.abs(Math.round(value)) })
            : t("lab.tip.report", {
                value: formatMs(e.value_ms),
                residual: e.residual_ms === null ? "–" : `${Math.round(e.residual_ms)} ms`,
                age: Math.round(e.age_ms ?? 0),
                category: t(cat.labelKey),
              });
        return { cx: x(e.t_ms, now), cy: y(value), cat, tip, off: Math.abs(value) > RANGE_MS };
      });
  });

  const markers = $derived.by(() => {
    if (!lab) return [];
    const now = lab.now_ms;
    return lab.entries
      .filter((e) => e.kind === "track" || e.kind === "status")
      .map((e) => ({ x: x(e.t_ms, now), label: t(e.kind === "track" ? "lab.marker.track" : "lab.marker.status") }));
  });

  const lastObs = $derived(lab ? [...lab.entries].reverse().find((e) => e.kind === "obs") : undefined);
  const recent = $derived(lab ? [...lab.entries].reverse().slice(0, 20) : []);
  const signed = (ms: number) => (ms > 0 ? `+${ms}` : ms < 0 ? `−${-ms}` : "0");

  async function exportNow() {
    exportPath = await ipc.exportDiagnostics();
  }

  onMount(() => {
    const poll = async () => {
      if (document.visibilityState === "visible") lab = await ipc.syncLab();
    };
    poll();
    const id = setInterval(poll, 250);
    return () => clearInterval(id);
  });
</script>

<section class="card">
  <h2>Sync Lab</h2>
  <p class="hint">{t("lab.intro")}</p>

  <ul class="legend">
    {#each CATEGORIES as c (c.id)}
      <li><span class="swatch {c.shape}" style="--c: {c.color}"></span>{t(c.labelKey)}</li>
    {/each}
  </ul>

  <svg viewBox="0 0 {W} {H}" role="img" aria-label={t("lab.chart")}>
    {#each [-200, -100, 0, 100, 200] as g (g)}
      <line class="grid" class:zero={g === 0} x1={PAD.l} x2={W - PAD.r} y1={y(g)} y2={y(g)} />
      <text class="axis" x={PAD.l - 6} y={y(g) + 4} text-anchor="end">{g === 0 ? "0" : signed(g)}</text>
    {/each}
    {#each [60, 45, 30, 15, 0] as s (s)}
      <text class="axis" x={PAD.l + (1 - s / WINDOW_S) * (W - PAD.l - PAD.r)} y={H - 6} text-anchor="middle">{s === 0 ? t("lab.axis.now") : `−${s} s`}</text>
    {/each}
    {#each markers as m, i (i)}
      <line class="marker" x1={m.x} x2={m.x} y1={PAD.t} y2={H - PAD.b}><title>{m.label}</title></line>
    {/each}
    {#each marks as m, i (i)}
      <g class="mark" style="--c: {m.cat.color}">
        <title>{m.tip}</title>
        <circle class="hit" cx={m.cx} cy={m.cy} r="9" />
        {#if m.cat.shape === "square"}
          <rect x={m.cx - 4} y={m.cy - 4} width="8" height="8" rx="1.5" />
        {:else if m.cat.shape === "triangle"}
          <path d="M {m.cx} {m.cy - 5} L {m.cx + 5} {m.cy + 4} L {m.cx - 5} {m.cy + 4} Z" />
        {:else if m.cat.shape === "diamond"}
          <path d="M {m.cx} {m.cy - 5} L {m.cx + 5} {m.cy} L {m.cx} {m.cy + 5} L {m.cx - 5} {m.cy} Z" />
        {:else}
          <circle cx={m.cx} cy={m.cy} r="4" />
        {/if}
      </g>
    {/each}
  </svg>

  {#if lab}
    <dl class="kv">
      <dt>{t("lab.position")}</dt>
      <dd>
        {formatMs(lab.clock.pos_ms)} · {t(lab.clock.playing ? "lab.position.running" : "lab.position.stopped")} ·
        {t("lab.position.rate", { rate: lab.clock.rate.toFixed(3) })}{lab.clock.slew_ms > 0
          ? ` · ${t("lab.position.slew", { ms: Math.round(lab.clock.slew_ms) })}`
          : ""}
      </dd>
      <dt>{t("lab.offsets")}</dt>
      <dd>
        {t("lab.offsets.value", {
          global: signed(lab.clock.offsets.global_ms),
          device: signed(lab.clock.offsets.device_ms),
          player: signed(lab.clock.offsets.player_ms),
          song: signed(lab.clock.offsets.song_ms),
          total: signed(lab.clock.offset_ms),
        })}
      </dd>
      <dt>{t("lab.reports")}</dt>
      <dd>
        {t("lab.reports.value", {
          cadence: lab.cadence_ms ? `${(lab.cadence_ms / 1000).toFixed(1)} s` : "–",
          age: lastObs?.age_ms != null ? `${Math.round(lastObs.age_ms)} ms` : "–",
          decision: lastObs?.update || "–",
        })}
        <!-- Browsers report only on play, pause, seek and song change (docs/spike-browser.md). -->
        {#if lab.browser}<br /><span class="hint">{t("lab.reports.browser")}</span>{/if}
      </dd>
      <dt>{t("lab.sources")}</dt>
      <dd>SMTC{lab.deviating.length ? ` · ${t("lab.sources.deviating", { list: lab.deviating.join(", ") })}` : ""}</dd>
      <dt>{t("lab.anchorSeq")}</dt>
      <dd>{lab.clock.seq}</dd>
    </dl>
  {/if}

  <div class="actions">
    <button onclick={() => (showTable = !showTable)}>{showTable ? t("lab.table.hide") : t("lab.table.show")}</button>
    <button onclick={() => ipc.openLogsFolder()}>{t("common.openLogs")}</button>
    <button class="primary" onclick={exportNow}>{t("lab.export")}</button>
  </div>
  {#if exportPath}<p class="hint mono">{exportPath}</p>{/if}

  {#if showTable && lab}
    <table>
      <thead>
        <tr>
          <th>{t("lab.col.ago")}</th>
          <th>{t("lab.col.kind")}</th>
          <th>{t("lab.col.value")}</th>
          <th>{t("lab.col.residual")}</th>
          <th>{t("lab.col.age")}</th>
          <th>{t("lab.col.decision")}</th>
        </tr>
      </thead>
      <tbody>
        {#each recent as e, i (i)}
          <tr>
            <td>{((lab.now_ms - e.t_ms) / 1000).toFixed(1)} s</td>
            <td>{e.kind}{e.source ? ` · ${e.source}` : ""}</td>
            <td>{e.kind === "obs" ? formatMs(e.value_ms) : `${Math.round(e.value_ms)} ms`}</td>
            <td>{e.residual_ms === null ? "–" : `${Math.round(e.residual_ms)} ms`}</td>
            <td>{e.age_ms === null ? "–" : `${Math.round(e.age_ms)} ms`}</td>
            <td>{e.update || "–"}</td>
          </tr>
        {/each}
      </tbody>
    </table>
  {/if}
</section>

<style>
  .card {
    background: var(--card);
    border: 1px solid var(--line);
    border-radius: 12px;
    padding: 16px 18px;
    margin-bottom: 14px;
  }
  h2 {
    margin: 0 0 8px;
    font-size: 15px;
  }
  .hint {
    margin: 0 0 8px;
    color: var(--muted);
    font-size: 12.5px;
  }
  .legend {
    display: flex;
    flex-wrap: wrap;
    gap: 6px 16px;
    margin: 8px 0;
    padding: 0;
    list-style: none;
    font-size: 12px;
    color: var(--muted);
  }
  .legend li {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .swatch {
    width: 9px;
    height: 9px;
    background: var(--c);
  }
  .swatch.circle {
    border-radius: 50%;
  }
  .swatch.square {
    border-radius: 2px;
  }
  .swatch.triangle {
    clip-path: polygon(50% 0, 100% 100%, 0 100%);
  }
  .swatch.diamond {
    transform: rotate(45deg) scale(0.85);
  }
  svg {
    width: 100%;
    height: auto;
    display: block;
    background: #15171c;
    border-radius: 8px;
  }
  .grid {
    stroke: rgba(255, 255, 255, 0.07);
    stroke-width: 1;
  }
  .grid.zero {
    stroke: rgba(255, 255, 255, 0.25);
  }
  .axis {
    fill: var(--muted);
    font-size: 11px;
  }
  .marker {
    stroke: rgba(255, 255, 255, 0.3);
    stroke-dasharray: 3 3;
  }
  .mark {
    fill: var(--c);
    stroke: #15171c;
    stroke-width: 2px;
  }
  .mark .hit {
    fill: transparent;
    stroke: none;
  }
  .mark:hover {
    filter: brightness(1.3);
  }
  .kv {
    display: grid;
    grid-template-columns: 170px 1fr;
    gap: 6px 12px;
    margin: 12px 0 0;
  }
  .kv dt {
    color: var(--muted);
  }
  .kv dd {
    margin: 0;
  }
  .actions {
    display: flex;
    gap: 8px;
    margin-top: 12px;
  }
  table {
    width: 100%;
    margin-top: 10px;
    border-collapse: collapse;
    font-size: 12px;
  }
  th,
  td {
    text-align: left;
    padding: 4px 6px;
    border-top: 1px solid var(--line);
  }
  th {
    color: var(--muted);
    font-weight: 500;
  }
</style>
