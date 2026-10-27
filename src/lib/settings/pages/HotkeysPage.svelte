<script lang="ts">
  // Settings → Hotkeys (PLAN.md 9.7, point 6): change or turn off each global shortcut, for
  // games that need the same keys. "Ändern" records the next key combination; meanwhile all
  // shortcuts are paused, so the old ones neither fire nor swallow the keys. A combination
  // needs Ctrl, Alt, Shift or Win, except F1–F12. The backend reports which ones it could
  // register (another program may own one).
  import { onMount } from "svelte";
  import { lang, t, type Key } from "#lib/shared/i18n.svelte.js";
  import { ipc } from "#lib/shared/ipc.js";
  import type { HotkeyAction, HotkeySettings, HotkeyStatus, Settings, SettingsPatch } from "#lib/shared/types.js";

  let { settings, update }: { settings: Settings; update: (patch: SettingsPatch) => Promise<void> } = $props();

  const ACTIONS: HotkeyAction[] = [
    "toggle",
    "edit",
    "layout",
    "romaji",
    "nudge_earlier",
    "nudge_later",
    "nudge_earlier_fine",
    "nudge_later_fine",
  ];
  const KEYS: Record<string, string> = {
    Comma: ",",
    Period: ".",
    Minus: "-",
    Slash: "/",
    Semicolon: ";",
    Quote: "'",
    BracketLeft: "[",
    BracketRight: "]",
    Backquote: "`",
    ArrowUp: "Up",
    ArrowDown: "Down",
    ArrowLeft: "Left",
    ArrowRight: "Right",
    Home: "Home",
    End: "End",
    PageUp: "PageUp",
    PageDown: "PageDown",
    Insert: "Insert",
    Delete: "Delete",
  };

  let status = $state<HotkeyStatus[]>([]);
  let recording = $state<HotkeyAction | null>(null);
  let hint = $state<string | null>(null);

  const stateOf = (a: HotkeyAction) => status.find((s) => s.action === a);

  /** "Ctrl+Alt+L" as the user reads it: Strg and Umschalt in German, Win for Super. */
  function shown(combo: string): string {
    if (!combo) return t("hotkeys.off");
    const parts = combo.split("+").map((p) => (p === "Super" ? "Win" : p));
    return lang() === "de" ? parts.map((p) => (p === "Ctrl" ? "Strg" : p === "Shift" ? "Umschalt" : p)).join("+") : parts.join("+");
  }

  /** The combination of a key press, or null for a lone modifier or an unsupported key. */
  function comboOf(e: KeyboardEvent): string | null {
    const code = e.code;
    let key: string | null = null;
    if (/^Key[A-Z]$/.test(code)) key = code.slice(3);
    else if (/^Digit\d$/.test(code)) key = code.slice(5);
    else if (/^Numpad\d$/.test(code)) key = `Num${code.slice(6)}`;
    else if (/^F([1-9]|1[0-2])$/.test(code)) key = code;
    else key = KEYS[code] ?? null;
    if (!key) return null;
    const mods = [e.ctrlKey && "Ctrl", e.altKey && "Alt", e.shiftKey && "Shift", e.metaKey && "Super"].filter(Boolean);
    return [...mods, key].join("+");
  }

  async function startRecording(a: HotkeyAction) {
    hint = null;
    recording = a;
    status = await ipc.suspendHotkeys(true);
  }

  async function stopRecording() {
    recording = null;
    status = await ipc.suspendHotkeys(false);
  }

  async function set(a: HotkeyAction, combo: string | null) {
    await update({ hotkeys: { [a]: combo } });
    status = await ipc.hotkeyStatus();
  }

  async function onKey(e: KeyboardEvent) {
    if (!recording) return;
    e.preventDefault();
    e.stopPropagation();
    if (e.code === "Escape") return stopRecording();
    if (["ControlLeft", "ControlRight", "AltLeft", "AltRight", "ShiftLeft", "ShiftRight", "MetaLeft", "MetaRight"].includes(e.code)) return;
    const combo = comboOf(e);
    const fKey = /^F([1-9]|1[0-2])$/.test(e.code);
    if (!combo || (!fKey && combo.indexOf("+") < 0)) {
      hint = t("hotkeys.needsModifier");
      return;
    }
    const action = recording;
    recording = null;
    await update({ hotkeys: { [action]: combo } });
    status = await ipc.suspendHotkeys(false);
  }

  onMount(() => {
    ipc.hotkeyStatus().then((s) => (status = s));
    window.addEventListener("keydown", onKey, true);
    // Leaving the page or the window while recording must not leave the shortcuts paused.
    const onBlur = () => recording && stopRecording();
    window.addEventListener("blur", onBlur);
    return () => {
      window.removeEventListener("keydown", onKey, true);
      window.removeEventListener("blur", onBlur);
      if (recording) ipc.suspendHotkeys(false);
    };
  });

  const resetAll = () => update({ hotkeys: Object.fromEntries(ACTIONS.map((a) => [a, null])) as { [K in keyof HotkeySettings]?: null } }).then(() => ipc.hotkeyStatus().then((s) => (status = s)));
</script>

<h1>{t("nav.hotkeys")}</h1>

<section class="card">
  <p class="hint top">{t("hotkeys.intro")}</p>
  <table>
    <tbody>
      {#each ACTIONS as a (a)}
        {@const st = stateOf(a)}
        <tr>
          <td class="action">{t(`hotkeys.action.${a}` as Key)}</td>
          <td class="combo">
            {#if recording === a}
              <span class="recording">{t("hotkeys.recording")}</span>
            {:else}
              <kbd class:off={!settings.hotkeys[a]}>{shown(settings.hotkeys[a])}</kbd>
            {/if}
          </td>
          <td class="state">
            {#if st && st.state !== "ok" && st.state !== "off" && recording !== a}
              <span class="bad">{t(`hotkeys.state.${st.state}`)}</span>
            {/if}
          </td>
          <td class="buttons">
            {#if recording === a}
              <button onclick={stopRecording}>{t("common.cancel")}</button>
            {:else}
              <button onclick={() => startRecording(a)} disabled={!!recording}>{t("hotkeys.change")}</button>
              <button onclick={() => set(a, "")} disabled={!!recording || !settings.hotkeys[a]}>{t("hotkeys.turnOff")}</button>
              <button onclick={() => set(a, null)} disabled={!!recording}>{t("hotkeys.default")}</button>
            {/if}
          </td>
        </tr>
      {/each}
    </tbody>
  </table>
  {#if hint}<p class="hint warn">{hint}</p>{/if}
  <div class="actions">
    <button onclick={resetAll} disabled={!!recording}>{t("hotkeys.resetAll")}</button>
  </div>
</section>

<section class="card">
  <h2>{t("hotkeys.fixed.title")}</h2>
  <p class="hint">{t("hotkeys.fixed.edit")}</p>
  <p class="hint">{t("hotkeys.fixed.measure")}</p>
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
  .hint {
    margin: 6px 0 0;
    color: var(--muted);
    font-size: 12.5px;
  }
  .hint.top {
    margin: 0 0 10px;
  }
  .warn,
  .bad {
    color: #f0b35a;
  }
  table {
    width: 100%;
    border-collapse: collapse;
  }
  td {
    padding: 7px 8px 7px 0;
    border-bottom: 1px solid var(--line);
    vertical-align: middle;
  }
  tr:last-child td {
    border-bottom: none;
  }
  .action {
    width: 38%;
  }
  .combo {
    width: 24%;
  }
  .state {
    font-size: 12.5px;
  }
  .buttons {
    text-align: right;
    white-space: nowrap;
  }
  .buttons button + button {
    margin-left: 6px;
  }
  kbd {
    display: inline-block;
    padding: 2px 8px;
    border: 1px solid var(--line);
    border-bottom-width: 2px;
    border-radius: 6px;
    background: #12151b;
    font: 12.5px/1.6 ui-monospace, "Cascadia Mono", Consolas, monospace;
  }
  kbd.off {
    color: var(--muted);
  }
  .recording {
    color: var(--accent);
    font-weight: 600;
  }
  .actions {
    margin-top: 12px;
  }
</style>
