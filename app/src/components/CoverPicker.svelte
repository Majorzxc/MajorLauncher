<script lang="ts">
  // Готовые обложки (раздел 6.2 ТЗ, design/covers.md): 12 картинок 48×48 —
  // клик ставит обложку. В мастере новой сборки и в «Изменить».
  import { DEFAULT_COVERS, defaultCover } from "../lib/cover";
  import { toast } from "../lib/toast.svelte";

  let { onpick }: { onpick: (png: string) => void } = $props();

  async function pick(url: string) {
    try {
      onpick(await defaultCover(url));
    } catch (e) {
      toast(`Обложка: ${e}`, { error: true });
    }
  }
</script>

<div class="covers" role="group" aria-label="Готовые обложки">
  {#each DEFAULT_COVERS as c (c.id)}
    <button title={c.title} aria-label="Обложка «{c.title}»" onclick={() => pick(c.url)}>
      <img src={c.url} alt="" width="48" height="48" />
    </button>
  {/each}
</div>

<style>
  .covers {
    display: grid;
    grid-template-columns: repeat(6, 48px);
    gap: 6px;
  }
  button {
    padding: 0;
    width: 48px;
    height: 48px;
    border: 1px solid var(--border);
    border-radius: var(--r-sm);
    overflow: hidden;
    background: none;
    cursor: pointer;
  }
  button:hover,
  button:focus-visible {
    border-color: var(--accent);
  }
  img {
    display: block;
    image-rendering: pixelated;
  }
</style>
