<script lang="ts" module>
  import type { IconName } from "../lib/icons";

  export interface MenuItem {
    label: string;
    icon: IconName;
    onclick?: () => void;
    /// Пункт ещё не работает — показываем серым с пояснением.
    hint?: string;
    danger?: boolean;
    /// Разделитель перед пунктом.
    divider?: boolean;
  }
</script>

<script lang="ts">
  import Icon from "./Icon.svelte";

  let { x, y, items, onclose }: { x: number; y: number; items: MenuItem[]; onclose: () => void } = $props();

  let menu: HTMLDivElement;
  let left = $state(0);
  let top = $state(0);

  // Меню не должно вылезать за край окна.
  $effect(() => {
    const r = menu.getBoundingClientRect();
    left = Math.min(x, window.innerWidth - r.width - 8);
    top = Math.min(y, window.innerHeight - r.height - 8);
    menu.querySelector<HTMLButtonElement>("button:not(:disabled)")?.focus();
  });
</script>

<svelte:window
  onkeydown={(e) => e.key === "Escape" && onclose()}
  onmousedown={(e) => !menu.contains(e.target as Node) && onclose()}
  onblur={onclose}
/>

<div class="menu" role="menu" bind:this={menu} style:left="{left}px" style:top="{top}px">
  {#each items as item}
    {#if item.divider}<hr />{/if}
    <button
      role="menuitem"
      class:danger={item.danger}
      disabled={!item.onclick}
      title={item.hint}
      onclick={() => {
        onclose();
        item.onclick?.();
      }}
    >
      <Icon name={item.icon} size={18} />
      <span>{item.label}</span>
      {#if item.hint}<small>{item.hint}</small>{/if}
    </button>
  {/each}
</div>

<style>
  .menu {
    position: fixed;
    z-index: 60;
    min-width: 220px;
    padding: var(--s1);
    background: var(--surface);
    border: 1px solid var(--border-strong);
    border-radius: var(--r);
  }
  button {
    display: flex;
    align-items: center;
    gap: var(--s3);
    width: 100%;
    height: 36px;
    padding: 0 var(--s3);
    border: 0;
    border-radius: var(--r-sm);
    background: none;
    text-align: left;
    font-size: var(--fs-14);
    cursor: pointer;
  }
  button:hover:not(:disabled),
  button:focus-visible {
    outline: none;
    background: var(--surface-raised);
  }
  button:disabled {
    color: var(--text-muted);
    cursor: default;
  }
  span {
    flex: 1;
  }
  small {
    font-size: var(--fs-12);
    color: var(--text-muted);
  }
  .danger {
    color: var(--danger);
  }
  hr {
    margin: var(--s1) 0;
    border: 0;
    border-top: 1px solid var(--border);
  }
</style>
