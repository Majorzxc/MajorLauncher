<script lang="ts">
  // Обложка сборки: своя картинка или пиксельный узор из id.
  import { builds } from "../lib/builds.svelte";
  import Identicon from "./Identicon.svelte";

  let { id, size }: { id: string; size: number } = $props();
  const src = $derived(builds.covers[id]);
</script>

<span class="cover" style:width="{size}px" style:height="{size}px">
  {#if src}<img {src} alt="" draggable="false" />{:else}<Identicon seed={id} />{/if}
</span>

<style>
  .cover {
    display: block;
    flex: none;
    overflow: hidden;
    border-radius: var(--r);
    background: var(--surface-raised);
  }
  img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    image-rendering: pixelated;
  }
</style>
