<script lang="ts">
  // Подсказка поверх окна, пока над ним держат файлы: куда они попадут.
  import { fade } from "svelte/transition";
  import { dropping } from "../lib/drop.svelte";
  import { ms } from "../lib/motion";
  import Icon from "./Icon.svelte";
</script>

{#if dropping.over}
  <div class="drop" transition:fade={{ duration: ms(100) }}>
    <p><Icon name="import" size={28} />{dropping.hint}</p>
  </div>
{/if}

<style>
  .drop {
    position: fixed;
    inset: 8px;
    z-index: 950;
    display: grid;
    place-items: center;
    border: 2px dashed var(--accent);
    border-radius: var(--r-card);
    background: color-mix(in srgb, var(--bg) 82%, transparent);
    pointer-events: none;
  }
  p {
    display: flex;
    align-items: center;
    gap: var(--s3);
    max-width: 560px;
    margin: 0;
    padding: var(--s4) var(--s5);
    border-radius: var(--r);
    background: var(--surface);
    border: 1px solid var(--border-strong);
    font-size: var(--fs-16);
    font-weight: 500;
  }
</style>
