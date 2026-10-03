<script lang="ts">
  import type { Snippet } from "svelte";
  import Icon from "./Icon.svelte";

  let {
    title,
    width = 560,
    onclose,
    children,
    footer,
  }: {
    title: string;
    width?: number;
    onclose: () => void;
    children: Snippet;
    footer?: Snippet;
  } = $props();

  let card: HTMLDivElement;

  $effect(() => {
    // Фокус внутрь окна — чтобы Tab и Esc работали сразу.
    card.querySelector<HTMLElement>("input, button:not(.close), [tabindex]")?.focus();
  });
</script>

<svelte:window onkeydown={(e) => e.key === "Escape" && onclose()} />

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
<div class="scrim" onmousedown={(e) => e.target === e.currentTarget && onclose()}>
  <div class="card" style:width="{width}px" bind:this={card} role="dialog" aria-modal="true" aria-label={title}>
    <header>
      <h2>{title}</h2>
      <button class="btn btn-quiet btn-sm close" aria-label="Закрыть" onclick={onclose}>
        <Icon name="close" size={18} />
      </button>
    </header>
    <div class="body">{@render children()}</div>
    {#if footer}<footer>{@render footer()}</footer>{/if}
  </div>
</div>

<style>
  .scrim {
    position: fixed;
    inset: 0;
    z-index: 50;
    display: grid;
    place-items: center;
    background: rgb(0 0 0 / 0.55);
  }
  .card {
    max-width: calc(100vw - 48px);
    max-height: calc(100vh - 48px);
    display: flex;
    flex-direction: column;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--r-card);
  }
  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: var(--s4) var(--s4) var(--s2) var(--s5);
  }
  h2 {
    margin: 0;
    font-size: var(--fs-20);
    font-weight: 600;
  }
  .body {
    padding: var(--s2) var(--s5) var(--s5);
    overflow: auto;
  }
  footer {
    display: flex;
    justify-content: flex-end;
    gap: var(--s2);
    padding: var(--s4) var(--s5);
    border-top: 1px solid var(--border);
  }
</style>
