<script lang="ts">
  import Icon from "./Icon.svelte";
  import { BOTTOM, TOP, type Section, type SectionId } from "../lib/sections";

  let { current, onselect }: { current: SectionId; onselect: (id: SectionId) => void } = $props();
</script>

{#snippet item(s: Section)}
  <button
    class="item"
    aria-current={current === s.id ? "page" : undefined}
    aria-label={s.title}
    onclick={() => onselect(s.id)}
  >
    <Icon name={s.icon} size={24} />
    <span class="tip">{s.title}</span>
  </button>
{/snippet}

<nav aria-label="Разделы">
  <div class="group">
    {#each TOP as s (s.id)}{@render item(s)}{/each}
  </div>
  <div class="group bottom">
    {#each BOTTOM as s (s.id)}{@render item(s)}{/each}
  </div>
</nav>

<style>
  nav {
    grid-area: 2 / 1;
    display: flex;
    flex-direction: column;
    background: var(--surface);
    border-right: 1px solid var(--border);
  }
  .group {
    display: flex;
    flex-direction: column;
  }
  .bottom {
    margin-top: auto;
    padding-bottom: var(--s2);
  }
  .item {
    position: relative;
    display: grid;
    place-items: center;
    height: var(--nav-item-h);
    border: 0;
    background: none;
    color: var(--text-muted);
    cursor: pointer;
  }
  .item:hover {
    color: var(--text);
    background: var(--surface-raised);
  }
  .item[aria-current="page"] {
    color: var(--text);
    background: var(--surface-raised);
  }
  .item[aria-current="page"]::before {
    content: "";
    position: absolute;
    left: 0;
    top: 0;
    bottom: 0;
    width: 4px;
    background: var(--accent);
  }
  .item:focus-visible {
    outline: none;
    box-shadow: inset 0 0 0 2px var(--accent);
  }

  /* Подсказка с названием раздела справа от иконки. */
  .tip {
    position: absolute;
    left: calc(100% + 8px);
    top: 50%;
    z-index: 10;
    padding: var(--s1) var(--s2);
    border: 1px solid var(--border);
    border-radius: var(--r-sm);
    background: var(--surface);
    color: var(--text);
    font-size: var(--fs-14);
    white-space: nowrap;
    pointer-events: none;
    opacity: 0;
    transform: translate(-4px, -50%);
    transition:
      opacity var(--t-fast) var(--ease),
      transform var(--t-fast) var(--ease);
  }
  .item:hover .tip,
  .item:focus-visible .tip {
    opacity: 1;
    transform: translate(0, -50%);
  }
</style>
