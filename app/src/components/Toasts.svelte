<script lang="ts">
  import Icon from "./Icon.svelte";
  import { dismiss, toasts } from "../lib/toast.svelte";
</script>

<div class="toasts" role="status" aria-live="polite">
  {#each toasts as t (t.id)}
    <div class="toast" class:error={t.error}>
      <Icon name={t.error ? "error" : "check"} size={18} />
      <span>{t.text}</span>
      {#if t.action}
        <button
          class="btn btn-sm"
          onclick={() => {
            t.action?.run();
            dismiss(t.id);
          }}>{t.action.label}</button
        >
      {/if}
      <button class="btn btn-quiet btn-sm x" aria-label="Закрыть" onclick={() => dismiss(t.id)}>
        <Icon name="close" size={16} />
      </button>
    </div>
  {/each}
</div>

<style>
  .toasts {
    position: fixed;
    right: var(--s5);
    bottom: var(--s5);
    z-index: 70;
    display: flex;
    flex-direction: column;
    gap: var(--s2);
    max-width: 420px;
  }
  .toast {
    display: flex;
    align-items: center;
    gap: var(--s3);
    padding: var(--s2) var(--s2) var(--s2) var(--s4);
    background: var(--surface);
    border: 1px solid var(--border-strong);
    border-radius: var(--r);
    font-size: var(--fs-14);
  }
  .toast.error {
    border-color: var(--danger);
  }
  .toast.error :global(.i) {
    color: var(--danger);
  }
  span {
    flex: 1;
  }
  .x {
    padding: 0 var(--s2);
  }
</style>
