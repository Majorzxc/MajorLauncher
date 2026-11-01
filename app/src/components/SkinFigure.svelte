<script lang="ts">
  // Скин фигурой во весь рост (2D, спереди) — для сеток библиотеки и каталога.
  import { figure } from "../lib/skinrender";

  let {
    key,
    load,
    slim = null,
    height = 128,
    onslim,
  }: {
    /// По нему кэшируется готовая фигура (id скина, адрес текстуры).
    key: string;
    load: () => Promise<ArrayBuffer>;
    /// `null` — определить по картинке.
    slim?: boolean | null;
    height?: number;
    /// Тонкие ли руки на самом деле (для каталога MineSkin).
    onslim?: (slim: boolean) => void;
  } = $props();

  let url = $state<string | null>(null);
  let failed = $state(false);

  $effect(() => {
    const k = key;
    failed = false;
    figure(k, load, slim)
      .then((f) => {
        if (k !== key) return;
        url = f.url;
        onslim?.(f.slim);
      })
      .catch(() => (failed = true));
  });
</script>

<div class="fig" style:height="{height}px" style:width="{height / 2}px">
  {#if url}
    <img src={url} alt="" />
  {:else if failed}
    <span class="muted small">нет картинки</span>
  {:else}
    <span class="ph"></span>
  {/if}
</div>

<style>
  .fig {
    display: grid;
    place-items: center;
  }
  img {
    width: 100%;
    height: 100%;
    image-rendering: pixelated;
    filter: drop-shadow(0 2px 3px rgb(0 0 0 / 0.35));
  }
  .ph {
    width: 60%;
    height: 80%;
    border-radius: var(--r);
    background: var(--surface-raised);
    animation: pulse 1.2s ease-in-out infinite;
  }
  .small {
    font-size: var(--fs-12);
    text-align: center;
  }
  @keyframes pulse {
    50% {
      opacity: 0.4;
    }
  }
</style>
