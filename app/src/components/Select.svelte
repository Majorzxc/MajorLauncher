<script lang="ts" generics="T">
  // Выпадающий список со своей анимацией: системный <select> не анимируется
  // и выглядит по-разному в тёмной и светлой теме.
  import { fly } from "svelte/transition";
  import type { IconName } from "../lib/icons";
  import { ms } from "../lib/motion";
  import Icon from "./Icon.svelte";

  let {
    value,
    options,
    onchange,
    label,
    icon,
    shrink = false,
  }: {
    value: T;
    options: { value: T; label: string }[];
    onchange: (v: T) => void;
    /// Для экранного диктора: «Сортировка», «Сборка».
    label: string;
    icon?: IconName;
    /// Длинные значения (названия сборок) — сжимать с многоточием.
    shrink?: boolean;
  } = $props();

  let open = $state(false);
  let root: HTMLDivElement;
  let list: HTMLUListElement | undefined = $state();
  const current = $derived(options.find((o) => o.value === value));

  function pick(v: T) {
    open = false;
    root.querySelector<HTMLButtonElement>(".trigger")?.focus();
    if (v !== value) onchange(v);
  }

  // При открытии фокус — на выбранном пункте, стрелки ходят по списку.
  $effect(() => {
    if (open && list) list.querySelector<HTMLButtonElement>("[aria-selected=true]")?.focus();
  });

  function keys(e: KeyboardEvent) {
    if (e.key !== "ArrowDown" && e.key !== "ArrowUp") return;
    e.preventDefault();
    const items = [...(list?.querySelectorAll<HTMLButtonElement>("button") ?? [])];
    const i = items.indexOf(document.activeElement as HTMLButtonElement);
    items[(i + (e.key === "ArrowDown" ? 1 : items.length - 1)) % items.length]?.focus();
  }
</script>

<svelte:window
  onmousedown={(e) => open && !root.contains(e.target as Node) && (open = false)}
  onkeydown={(e) => open && e.key === "Escape" && (open = false)}
/>

<div class="select" class:shrink bind:this={root}>
  <button
    class="trigger"
    aria-haspopup="listbox"
    aria-expanded={open}
    aria-label="{label}: {current?.label ?? ''}"
    title={current?.label}
    onclick={() => (open = !open)}
  >
    {#if icon}<Icon name={icon} size={18} />{/if}
    <span class="value">{current?.label ?? ""}</span>
    <span class="chevron" class:up={open}><Icon name="chevron-down" size={16} /></span>
  </button>
  {#if open}
    <ul
      class="list"
      role="listbox"
      aria-label={label}
      bind:this={list}
      onkeydown={keys}
      in:fly={{ y: -4, duration: ms(120) }}
      out:fly={{ y: -4, duration: ms(80) }}
    >
      {#each options as o, i (i)}
        <li>
          <button role="option" aria-selected={o.value === value} title={o.label} onclick={() => pick(o.value)}>
            <span class="grow">{o.label}</span>
            {#if o.value === value}<Icon name="check" size={16} />{/if}
          </button>
        </li>
      {/each}
    </ul>
  {/if}
</div>

<style>
  .select {
    position: relative;
  }
  .shrink {
    min-width: 0;
  }
  .shrink .value {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .trigger {
    display: flex;
    align-items: center;
    gap: var(--s2);
    width: 100%;
    height: var(--h-md);
    padding: 0 var(--s3);
    border: 1px solid var(--border);
    border-radius: var(--r);
    background: var(--surface);
    color: var(--text-muted);
    cursor: pointer;
  }
  .trigger:hover,
  .trigger[aria-expanded="true"] {
    border-color: var(--border-strong);
  }
  .value {
    flex: 1;
    color: var(--text);
    text-align: left;
    white-space: nowrap;
  }
  .chevron {
    display: inline-flex;
    transition: transform var(--t-fast) var(--ease);
  }
  .chevron.up {
    transform: rotate(180deg);
  }
  .list {
    position: absolute;
    top: calc(100% + 6px);
    left: 0;
    z-index: 60;
    min-width: 100%;
    max-width: min(560px, 90vw);
    max-height: 320px;
    margin: 0;
    padding: var(--s1);
    overflow: auto;
    list-style: none;
    background: var(--surface);
    border: 1px solid var(--border-strong);
    border-radius: var(--r);
  }
  .list button {
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
    white-space: nowrap;
    cursor: pointer;
  }
  .list .grow {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .list button:hover,
  .list button:focus-visible {
    outline: none;
    background: var(--surface-raised);
  }
</style>
