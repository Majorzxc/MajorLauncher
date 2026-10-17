<script lang="ts">
  // Раздел «Моды»: каталог Modrinth и библиотека (раздел 5.4 ТЗ).
  import { catalog } from "../lib/catalog.svelte";
  import Catalog from "./Catalog.svelte";
  import Library from "./Library.svelte";
</script>

<div class="views" role="tablist" aria-label="Каталог или библиотека">
  <button class="chip" role="tab" aria-selected={catalog.view === "catalog"} class:on={catalog.view === "catalog"} onclick={() => (catalog.view = "catalog")}>
    Каталог Modrinth
  </button>
  <button class="chip" role="tab" aria-selected={catalog.view === "library"} class:on={catalog.view === "library"} onclick={() => (catalog.view = "library")}>
    Библиотека
  </button>
</div>

{#if catalog.view === "catalog"}
  <Catalog />
{:else}
  <Library />
{/if}


<style>
  .views {
    display: flex;
    gap: var(--s2);
    margin-bottom: var(--s4);
  }
</style>
