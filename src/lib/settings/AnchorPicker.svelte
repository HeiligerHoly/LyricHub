<script lang="ts">
  // 3×3 picker for the nine anchor points (PLAN.md 6.4).
  import { t } from "$lib/shared/i18n.svelte";
  import { ANCHORS, type Anchor } from "$lib/shared/types";

  let { value, onpick }: { value: Anchor; onpick: (a: Anchor) => void } = $props();

  const name = (a: Anchor) => t(`anchor.${a}`);
</script>

<div class="grid" role="radiogroup" aria-label={t("anchor.group")}>
  {#each ANCHORS.flat() as a (a)}
    <button
      class="cell"
      class:active={value === a}
      role="radio"
      aria-checked={value === a}
      title={name(a)}
      aria-label={name(a)}
      onclick={() => onpick(a)}
    >
      <span class="dot"></span>
    </button>
  {/each}
</div>
<span class="name">{name(value)}</span>

<style>
  .grid {
    display: inline-grid;
    grid-template-columns: repeat(3, 34px);
    grid-template-rows: repeat(3, 22px);
    gap: 4px;
    padding: 6px;
    border-radius: 10px;
    background: #12151b;
    border: 1px solid var(--line);
    aspect-ratio: auto;
  }
  .cell {
    display: grid;
    place-items: center;
    padding: 0;
    border-radius: 5px;
    background: #1f232c;
  }
  .dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--muted);
  }
  .cell.active {
    background: var(--accent);
    border-color: transparent;
  }
  .cell.active .dot {
    background: var(--accent-ink);
  }
  .name {
    margin-left: 12px;
    color: var(--muted);
  }
</style>
